//! Cargo builds two of which can be compared: what a build produced, read from the JSON messages
//! of `cargo build --message-format=json-render-diagnostics` for packages built from a path:
//!
//!   rmeta   every .rmeta, by path
//!   rlib    every .rlib's members by name, session suffixes removed (`artifacts`)
//!   exe     every executable
//!   diag    every diagnostic, rendered, counted per crate
//!
//! Also what the compiler's own checks print (RUSTC_VERIFY_REUSE, RUSTC_REPORT_UNTRACKED), and
//! the process-group timeout and tree copies the incremental fuzzers need.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde::Deserialize;
use wait_timeout::ChildExt;

use crate::artifacts::{normalized_rlib, sha256};

#[derive(Deserialize)]
pub struct Message {
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub package_id: String,
    #[serde(default)]
    pub target: Option<Target>,
    #[serde(default)]
    pub filenames: Vec<String>,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub fresh: bool,
    #[serde(default)]
    pub message: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct Target {
    pub name: String,
    #[serde(default)]
    pub kind: Vec<String>,
}

impl Message {
    pub fn target_name(&self) -> &str {
        self.target.as_ref().map_or("", |t| t.name.as_str())
    }
    pub fn from_path(&self) -> bool {
        self.package_id.contains("path+file")
    }
}

/// Cargo's JSON messages in `stdout`, skipping lines that are not.
pub fn messages(stdout: &str) -> impl Iterator<Item = Message> + '_ {
    stdout.lines().filter_map(|l| serde_json::from_str(l).ok())
}

/// `f` relative to `target` when it is inside it.
pub fn relative(f: &str, target: &Path) -> String {
    Path::new(f).strip_prefix(target).map_or_else(|_| f.to_owned(), |p| p.to_string_lossy().into_owned())
}

#[derive(Default, Clone, PartialEq)]
pub struct Collected {
    pub rmeta: BTreeMap<String, String>,
    pub rlib: BTreeMap<String, BTreeMap<String, String>>,
    pub exe: BTreeMap<String, String>,
    /// (crate, rendered diagnostic) -> count
    pub diag: BTreeMap<(String, String), usize>,
}

static SESSION: LazyLock<regex::bytes::Regex> =
    LazyLock::new(|| regex::bytes::Regex::new(r"\.[0-9a-z]{7}(\.rcgu\.(?:o|dwo))").unwrap());

/// Artifacts from cargo's JSON messages, keyed by path relative to `target`.
pub fn collect(stdout: &str, target: &Path) -> Collected {
    let mut found = Collected::default();
    for msg in messages(stdout) {
        if !msg.from_path() {
            continue;
        }
        if msg.reason == "compiler-message" {
            let m = msg.message.as_ref();
            let text = m
                .and_then(|m| m.get("rendered").and_then(|r| r.as_str()).filter(|s| !s.is_empty()))
                .or_else(|| m.and_then(|m| m.get("message")).and_then(|r| r.as_str()))
                .unwrap_or("");
            *found.diag.entry((msg.target_name().to_owned(), text.to_owned())).or_default() += 1;
            continue;
        }
        if msg.reason != "compiler-artifact" {
            continue;
        }
        for f in &msg.filenames {
            let rel = relative(f, target);
            if f.ends_with(".rmeta") {
                found.rmeta.insert(rel, sha256(&std::fs::read(f).unwrap_or_default()));
            } else if f.ends_with(".rlib") {
                found.rlib.insert(rel, normalized_rlib(Path::new(f)));
            }
        }
        if let Some(f) = &msg.executable {
            let data = std::fs::read(f).unwrap_or_default();
            found.exe.insert(relative(f, target), sha256(&SESSION.replace_all(&data, &b"$1"[..])));
        }
    }
    found
}

fn differing<V: PartialEq>(a: &BTreeMap<String, V>, b: &BTreeMap<String, V>) -> Vec<String> {
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    keys.into_iter().filter(|k| a.get(*k) != b.get(*k)).cloned().collect()
}

/// {kind: [what differs]} for the kinds that differ between two collections.
pub fn compare(a: &Collected, b: &Collected) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (kind, x, y) in [("rmeta", &a.rmeta, &b.rmeta), ("exe", &a.exe, &b.exe)] {
        let d = differing(x, y);
        if !d.is_empty() {
            out.insert(kind.to_owned(), d);
        }
    }
    let empty = BTreeMap::new();
    let mut rlib = Vec::new();
    for rel in a.rlib.keys().chain(b.rlib.keys()).collect::<BTreeSet<_>>() {
        let members = differing(a.rlib.get(rel).unwrap_or(&empty), b.rlib.get(rel).unwrap_or(&empty));
        if !members.is_empty() {
            let more = if members.len() > 5 { " …" } else { "" };
            rlib.push(format!("{rel}: {}{more}", members[..members.len().min(5)].join(", ")));
        }
    }
    if !rlib.is_empty() {
        out.insert("rlib".to_owned(), rlib);
    }
    if a.diag != b.diag {
        let only = |x: &BTreeMap<(String, String), usize>, y: &BTreeMap<(String, String), usize>, which: &str| {
            x.iter()
                .filter(|(k, n)| y.get(*k).copied().unwrap_or(0) < **n)
                .map(|((c, t), _)| format!("{c} only in the {which}: {:?}", t.chars().take(200).collect::<String>()))
                .collect::<Vec<_>>()
        };
        let mut d = only(&a.diag, &b.diag, "first");
        d.extend(only(&b.diag, &a.diag, "second"));
        out.insert("diag".to_owned(), d);
    }
    out
}

/// What the compiler's own check of reused results (docs/hunt/verify-reuse.patch) found stale:
/// `query <name>`, `metadata`, `codegen unit` or `allocation sharing <queries>`, once each.
pub fn reuse_checks(log: &str) -> Vec<String> {
    static QUERY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"query `(\w+)`").unwrap());
    static MODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"TypingModeEqWrapper\((\w+)\)").unwrap());
    let mut found = BTreeSet::new();
    for line in log.lines() {
        if line.starts_with("rustc-verify-reuse: query `") {
            found.insert(format!("query {}", line.split('`').nth(1).unwrap_or("")));
        } else if line.starts_with("rustc-verify-reuse: metadata") {
            found.insert("metadata".to_owned());
        } else if line.starts_with("rustc-verify-reuse: codegen unit") {
            found.insert("codegen unit".to_owned());
        } else if line.starts_with("rustc-verify-reuse: allocation shared differently") {
            // Named by the two queries and typing modes, so that a new pattern is kept apart
            // from a known one.
            let mut parts: Vec<String> = QUERY.captures_iter(line).map(|c| c[1].to_owned()).collect();
            let modes: BTreeSet<String> = MODE.captures_iter(line).map(|c| c[1].to_owned()).collect();
            parts.extend(modes);
            found.insert(format!("allocation sharing {}", parts.join(" / ")));
        }
    }
    found.into_iter().collect()
}

/// Reads of untracked state the compiler reported (docs/hunt/report-untracked.patch).
pub fn untracked_reads(log: &str) -> BTreeSet<String> {
    log.lines().filter(|l| l.starts_with("rustc-untracked-read:")).map(|l| l.trim().to_owned()).collect()
}

/// Adds new reports of untracked reads to `path`, which lists each once.
pub fn note_untracked(path: &Path, lines: &BTreeSet<String>) {
    let known: BTreeSet<String> = std::fs::read_to_string(path).unwrap_or_default().lines().map(str::to_owned).collect();
    let new: String = lines.difference(&known).map(|l| format!("{l}\n")).collect();
    if !new.is_empty() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(new.as_bytes());
        }
    }
}

/// Whether a compiler log shows a crash.
pub fn is_ice(log: &str) -> bool {
    log.contains("internal compiler error") || log.contains("the compiler unexpectedly panicked")
}

/// How a build command ended.
pub struct Run {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    /// Killed at the timeout while a rustc was still running.
    pub hang: bool,
}

/// Run `cmd` in its own process group; after `timeout` the group is killed, and the run counts
/// as a hang if a rustc was still running in it (a looping build script is the fixture's
/// problem, not the compiler's).
pub fn run_group(mut cmd: Command, timeout: Option<Duration>) -> std::io::Result<Run> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    let mut child = cmd.spawn()?;
    let pid = child.id() as i32;
    let (mut out, mut err) = (child.stdout.take().expect("piped"), child.stderr.take().expect("piped"));
    let out_thread = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = out.read_to_end(&mut s);
        s
    });
    let err_thread = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err.read_to_end(&mut s);
        s
    });
    let status = match timeout {
        Some(t) => child.wait_timeout(t)?,
        None => Some(child.wait()?),
    };
    let mut note = String::new();
    let (ok, hang) = match status {
        Some(s) => (s.success(), false),
        None => {
            let ps = Command::new("ps").args(["-o", "args=", "-g", &pid.to_string()]).output();
            let group = ps.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
            let hang = group.lines().filter_map(|l| l.split_whitespace().next()).any(|a| a.ends_with("/rustc"));
            // SAFETY: signalling the process group we created.
            unsafe { libc::kill(-pid, libc::SIGKILL) };
            let _ = child.wait();
            note = format!("\nkilled after {}s; still running:\n{group}", timeout.unwrap().as_secs());
            (false, hang)
        }
    };
    let stdout = String::from_utf8_lossy(&out_thread.join().unwrap_or_default()).into_owned();
    let mut stderr = String::from_utf8_lossy(&err_thread.join().unwrap_or_default()).into_owned();
    stderr.push_str(&note);
    Ok(Run { ok, stdout, stderr, hang })
}

/// The last `n` characters of `s`.
pub fn tail(s: &str, n: usize) -> &str {
    let count = s.chars().count();
    if count <= n {
        return s;
    }
    let skip = s.char_indices().nth(count - n).map_or(0, |(i, _)| i);
    &s[skip..]
}

/// Copy a tree, skipping entries named in `ignore` at any depth, keeping modification times
/// (Cargo's freshness depends on them). Symbolic links are copied as links with `symlinks`,
/// followed otherwise.
pub fn copy_tree(from: &Path, to: &Path, ignore: &[&str], symlinks: bool) -> std::io::Result<()> {
    let walk = walkdir::WalkDir::new(from).follow_links(!symlinks).into_iter().filter_entry(|e| {
        e.depth() == 0 || !ignore.iter().any(|i| e.file_name() == *i)
    });
    for e in walk {
        let e = e.map_err(std::io::Error::other)?;
        let dest: PathBuf = to.join(e.path().strip_prefix(from).unwrap());
        let ft = e.file_type();
        if ft.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(e.path())?, &dest)?;
        } else if ft.is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            std::fs::copy(e.path(), &dest)?;
            let meta = e.metadata().map_err(std::io::Error::other)?;
            let times = std::fs::FileTimes::new().set_modified(meta.modified()?).set_accessed(meta.accessed()?);
            std::fs::File::options().write(true).open(&dest)?.set_times(times)?;
        }
    }
    Ok(())
}

/// Free bytes on the file system holding `path`.
pub fn free_bytes(path: &Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return u64::MAX };
    // SAFETY: statvfs is plain data, filled by the call for a valid C string.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return u64::MAX;
    }
    s.f_bavail as u64 * s.f_frsize as u64
}

//! Fold coverage logs (MIRTH_OUT, from a compiler built with rustc/coverage.toml) into a running
//! union as they are finished, and delete them: a test suite run starts tens of thousands of rustc
//! processes, whose logs together would not fit on the disk.
//!
//! A log is finished when its last line is the `X` line written at exit, or when it has not
//! changed for ten minutes (a process that crashed). For each, <out>/added.jsonl gets the
//! process's source file argument and the sites it reached that no earlier process did;
//! <out>/union.txt holds every site reached so far, rewritten every pass. Runs until <until>
//! exists, then does a last pass.
//!
//! The dimensions beyond blocks (rustc/coverage-dims.toml, docs/coverage-plan.md) fold beside it:
//! keyed sites (`K` lines) into keyed.txt as `<site>\t<key>`, call pairs (`D` lines) into
//! pairs.txt as `<caller>\t<callee>`, and the files the coverage patch writes into the logs'
//! directory (`<pid>.passes` from RUSTC_PASS_EFFECT, `<pid>.locks` from RUSTC_LOCK_CONTENTION)
//! into passes.txt and locks.txt, once they have not changed for a minute.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime};

use mirth_lab::coverage::{files_with, log_hits, to_json_line};
use regex::Regex;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    logs: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = "")]
    until: String,
}

static CRATE_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"--crate-name\t(\S+)").unwrap());

/// The process's source file argument, from the log's header line, or its crate name.
fn source_of(header: &str) -> String {
    if let Some(f) = header.split('\t').skip(3).find(|f| f.ends_with(".rs")) {
        return f.to_owned();
    }
    CRATE_NAME.captures(header).map(|c| c[1].to_owned()).unwrap_or_default()
}

#[derive(serde::Serialize)]
struct Added<'a> {
    source: String,
    new: &'a [&'a str],
}

/// The unions beside union.txt, each one line per distinct record.
#[derive(Default)]
struct Extra {
    keyed: BTreeSet<String>,
    pairs: BTreeSet<String>,
    passes: BTreeSet<String>,
    locks: BTreeSet<String>,
}

impl Extra {
    const FILES: [&str; 4] = ["keyed.txt", "pairs.txt", "passes.txt", "locks.txt"];

    fn load(out: &Path) -> Extra {
        let read = |name: &str| -> BTreeSet<String> {
            std::fs::read_to_string(out.join(name)).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(str::to_owned).collect()
        };
        Extra { keyed: read(Self::FILES[0]), pairs: read(Self::FILES[1]), passes: read(Self::FILES[2]), locks: read(Self::FILES[3]) }
    }

    fn from_log(&mut self, text: &str) {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("K\t") {
                // `<combined> <site> <key>`: the combined site is a V line too.
                if let Some((_, parts)) = rest.split_once('\t') {
                    self.keyed.insert(parts.to_owned());
                }
            } else if let Some(rest) = line.strip_prefix("D\t") {
                self.pairs.insert(rest.to_owned());
            }
        }
    }

    /// The coverage patch's files, folded once they have not changed for a minute (or at the
    /// end), then deleted.
    fn from_patch_files(&mut self, logs: &Path, last: bool) {
        for (ext, set, fields) in [("passes", &mut self.passes, 4), ("locks", &mut self.locks, 3)] {
            for file in files_with(logs, ext) {
                let Ok(meta) = std::fs::metadata(&file) else { continue };
                let age = meta.modified().ok().and_then(|m| SystemTime::now().duration_since(m).ok()).unwrap_or_default().as_secs_f64();
                if !(age > 60.0 || (last && age > 5.0)) {
                    continue;
                }
                for line in std::fs::read_to_string(&file).unwrap_or_default().lines() {
                    let kept: Vec<&str> = line.split('\t').take(fields).collect();
                    if kept.len() == fields {
                        set.insert(kept.join("\t"));
                    }
                }
                let _ = std::fs::remove_file(&file);
            }
        }
    }

    fn write(&self, out: &Path) -> anyhow::Result<()> {
        for (name, set) in Self::FILES.iter().zip([&self.keyed, &self.pairs, &self.passes, &self.locks]) {
            if set.is_empty() {
                continue;
            }
            let mut text = set.iter().map(String::as_str).collect::<Vec<_>>().join("\n");
            text.push('\n');
            std::fs::write(out.join(name), text)?;
        }
        Ok(())
    }
}

fn one_pass(logs: &Path, out: &Path, union: &mut BTreeSet<String>, extra: &mut Extra, last: bool) -> anyhow::Result<usize> {
    let mut done = 0;
    let mut added = std::fs::OpenOptions::new().create(true).append(true).open(out.join("added.jsonl"))?;
    for log in files_with(logs, "log") {
        let (Ok(bytes), Ok(meta)) = (std::fs::read(&log), std::fs::metadata(&log)) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let age = meta.modified().ok().and_then(|m| SystemTime::now().duration_since(m).ok()).unwrap_or_default().as_secs_f64();
        let Some(last_line) = text.lines().last() else { continue };
        if !(last_line.starts_with("X\t") || age > 600.0 || (last && age > 5.0)) {
            continue;
        }
        extra.from_log(&text);
        let sites: BTreeSet<&str> = log_hits(&text).collect();
        let new: Vec<&str> = sites.into_iter().filter(|s| !union.contains(*s)).collect();
        if !new.is_empty() {
            let line = to_json_line(&Added { source: source_of(text.lines().next().unwrap_or("")), new: &new });
            writeln!(added, "{line}")?;
            union.extend(new.iter().map(|s| s.to_string()));
        }
        let _ = std::fs::remove_file(&log);
        done += 1;
    }
    let mut text = union.iter().map(String::as_str).collect::<Vec<_>>().join("\n");
    text.push('\n');
    std::fs::write(out.join("union.txt"), text)?;
    extra.from_patch_files(logs, last);
    extra.write(out)?;
    Ok(done)
}

/// The local time as HH:MM:SS.
fn clock() -> String {
    // SAFETY: time and localtime_r write only to the locals passed.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.out)?;
    let union_path = args.out.join("union.txt");
    let mut union: BTreeSet<String> = std::fs::read_to_string(&union_path).unwrap_or_default().split_whitespace().map(str::to_owned).collect();
    let mut extra = Extra::load(&args.out);
    loop {
        let finishing = !args.until.is_empty() && Path::new(&args.until).exists();
        let n = one_pass(&args.logs, &args.out, &mut union, &mut extra, finishing)?;
        println!("{} {n} logs folded, {} sites", clock(), union.len());
        if finishing {
            one_pass(&args.logs, &args.out, &mut union, &mut extra, true)?;
            break;
        }
        std::thread::sleep(Duration::from_secs(30));
    }
    Ok(ExitCode::SUCCESS)
}

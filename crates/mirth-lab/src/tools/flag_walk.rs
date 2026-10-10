//! Walk option transitions on a fixture: for each row of a PICT table made from a
//! `flag-model --transitions` model, build the fixture clean with the A_ options, rebuild it
//! incrementally with the B_ options, build it clean with the B_ options, and compare the
//! rebuild with the clean build (metadata, object code, binary, diagnostics, the binary's
//! output), as the fuzzer does after an edit.
//!
//! Rows change options only, unless --edits asks for random source edits between A and B.
//!
//! The options go in RUSTFLAGS with `--target` set, so they apply to the fixture's crates but
//! not to its build scripts and proc macros. RUSTC_VERIFY_REUSE and RUSTC_REPORT_UNTRACKED are
//! set, for a compiler with mirth's local patches.
//!
//! Writes <work>/results.jsonl (one line per row) and <work>/findings/<row>/ for each row whose
//! rebuild differs from the clean build, or which crashed.
//!
//! To stay at the frontier: with --pause-on-finding the walk stops taking rows at the first
//! finding not marked known and writes <work>/PAUSED. Patch the compiler, then run the same
//! command with --rustc <patched> --recheck: the rows with findings run again first, then the
//! rows not yet walked. A rerun never repeats rows that are done.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use rand::rngs::StdRng;
use rand::{Rng as _, SeedableRng};
use serde_json::Value;

use mirth_lab::coverage::to_json_indent;
use super::flag_model::{self, Opt};
use mirth_lab::artifacts::{self, Collected};
use mirth_lab::mutations;
use mirth_lab::rustc::{run_command, Exit};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: String,
    #[arg(long)]
    fixture: PathBuf,
    /// flag-universe's work directory.
    #[arg(long)]
    flags: PathBuf,
    #[arg(long)]
    table: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value_t = 8)]
    workers: usize,
    #[arg(long, default_value = "nightly-2026-10-06")]
    toolchain: String,
    #[arg(long, default_value = "x86_64-unknown-linux-gnu")]
    target: String,
    #[arg(long, default_value_t = 600)]
    timeout: u64,
    /// a:b, a slice of the table.
    #[arg(long, default_value = "")]
    rows: String,
    /// Random source edits between A and B (the fuzzer's).
    #[arg(long, default_value_t = 0)]
    edits: usize,
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Stop taking rows at the first finding not marked known; rerun to resume.
    #[arg(long)]
    pause_on_finding: bool,
    /// On resume, run the rows that had findings again first (after patching rustc).
    #[arg(long)]
    recheck: bool,
    /// Clean rebuilds before a difference counts as reuse.
    #[arg(long, default_value_t = 12)]
    p5_builds: usize,
}

/// Copy a fixture, leaving out build output and edit scratch at any depth.
pub fn copy_fixture(from: &Path, to: &Path) -> std::io::Result<()> {
    let skip = |n: &std::ffi::OsStr| n == "target" || n == "edits" || n == "edit";
    for e in walkdir::WalkDir::new(from).follow_links(true).into_iter().filter_entry(|e| e.depth() == 0 || !skip(e.file_name())) {
        let e = e?;
        let dest = to.join(e.path().strip_prefix(from).expect("under the fixture"));
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            std::fs::copy(e.path(), &dest)?;
        }
    }
    Ok(())
}

/// Python's slice `a:b` of 0..n.
/// Rows `lo:hi` (Python slice bounds, either may be empty or negative); empty means all.
pub fn slice(spec: &str, n: usize) -> anyhow::Result<Vec<usize>> {
    if spec.is_empty() {
        return Ok((0..n).collect());
    }
    let Some((lo, hi)) = spec.split_once(':') else { anyhow::bail!("--rows takes lo:hi, not {spec:?}") };
    let at = |s: &str, default: usize| -> anyhow::Result<usize> {
        if s.is_empty() {
            return Ok(default);
        }
        let v: i64 = s.parse().map_err(|_| anyhow::anyhow!("--rows takes lo:hi, not {spec:?}"))?;
        Ok(if v < 0 { (n as i64 + v).max(0) as usize } else { (v as usize).min(n) })
    };
    Ok((at(lo, 0)?..at(hi, n)?).collect())
}

/// Python's repr of a string, as the findings have always shown texts.
pub fn py_repr(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::from(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

/// Python's repr of a JSON value (lists of strings and numbers, None).
pub fn py_value(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::String(s) => py_repr(s),
        Value::Array(a) => format!("[{}]", a.iter().map(py_value).collect::<Vec<_>>().join(", ")),
        other => other.to_string(),
    }
}

fn py_list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| py_repr(s)).collect::<Vec<_>>().join(", "))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RowResult {
    pub row: usize,
    #[serde(rename = "A")]
    pub a: Vec<String>,
    #[serde(rename = "B")]
    pub b: Vec<String>,
    #[serde(rename = "A_ok")]
    pub a_ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inc_ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clean_ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untracked: Option<Vec<String>>,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub findings: Vec<String>,
    #[serde(default)]
    pub rustc: String,
}

struct Build {
    ok: bool,
    log: String,
    ice: bool,
    reuse: Vec<String>,
    untracked: Vec<String>,
    error: String,
    errors: Vec<String>,
    art: Option<Collected>,
    exe: Option<String>,
}

struct Walk<'a> {
    args: &'a Args,
    opts: BTreeMap<String, Opt>,
    fixture: PathBuf,
    work: PathBuf,
    results: Mutex<()>,
}

fn tail(s: &str, n: usize) -> &str {
    let mut i = s.len().saturating_sub(n);
    while !s.is_char_boundary(i) {
        i += 1;
    }
    &s[i..]
}

impl Walk<'_> {
    /// RUSTFLAGS for one side of a row. Cargo passes `-Cembed-bitcode=no` unless its profile
    /// asks for LTO, and the profile's LTO reaches only the final artifacts, so `-Clto` goes in
    /// RUSTFLAGS with `-Cembed-bitcode=yes` after Cargo's flag.
    fn flags(&self, row: &[(String, String)], side: &str) -> Vec<String> {
        let mut out = flag_model::row_flags(row, Some(side), &self.opts);
        let lto = flag_model::get(row, &format!("{side}_Clto"));
        if matches!(lto, Some("yes" | "on" | "thin" | "fat")) && flag_model::get(row, &format!("{side}_Cembed_bitcode")) == Some("absent") {
            out.push("-Cembed-bitcode=yes".into());
        }
        out
    }

    fn build(&self, src: &Path, target: &Path, rustflags: &[String]) -> Build {
        let mut cmd = Command::new("cargo");
        cmd.arg(format!("+{}", self.args.toolchain))
            .args(["build", "--workspace", "--offline", "-j", "4", "--target", &self.args.target, "--target-dir"])
            .arg(target)
            .arg("--message-format=json-render-diagnostics")
            .current_dir(src)
            .env("RUSTC", &self.args.rustc)
            .env("RUSTC_WRAPPER", "")
            .env("CARGO_INCREMENTAL", "1")
            .env("CARGO_TERM_COLOR", "never")
            .env("RUSTFLAGS", rustflags.join(" "))
            .env("RUSTC_VERIFY_REUSE", "1")
            .env("RUSTC_REPORT_UNTRACKED", "1");
        let (ok, out, log) = match run_command(cmd, Duration::from_secs(self.args.timeout)) {
            Ok(f) => {
                let mut log = f.stderr_text();
                if f.exit == Exit::Timeout {
                    log.push_str(&format!("\nkilled after {}s", self.args.timeout));
                }
                (f.success(), f.stdout_text(), log)
            }
            Err(e) => (false, String::new(), e.to_string()),
        };
        let name = self.fixture.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut exe = None;
        for line in out.lines() {
            let Ok(msg) = serde_json::from_str::<Value>(line) else { continue };
            if msg["reason"] == "compiler-artifact" && msg["target"]["name"] == name.as_str() {
                if let Some(e) = msg["executable"].as_str().filter(|e| !e.is_empty()) {
                    exe = Some(e.to_owned());
                }
            }
        }
        let mut reuse: Vec<String> = log
            .lines()
            .filter(|l| l.starts_with("rustc-verify-reuse:"))
            .map(|l| l.split_once(':').unwrap().1.trim().chars().take(200).collect())
            .collect();
        reuse.sort();
        reuse.dedup();
        let mut untracked: Vec<String> = log.lines().filter(|l| l.starts_with("rustc-untracked-read:")).map(|l| l.trim().to_owned()).collect();
        untracked.sort();
        untracked.dedup();
        Build {
            ok,
            ice: log.contains("internal compiler error") || log.contains("the compiler unexpectedly panicked") || log.contains("rustc interrupted by SIG"),
            reuse,
            untracked,
            error: log.lines().find(|l| l.starts_with("error")).unwrap_or("").to_owned(),
            errors: log
                .lines()
                .filter(|l| (l.starts_with("error") || l.starts_with("rustc-LLVM ERROR") || l.starts_with("LLVM ERROR")) && !l.contains("could not compile"))
                .take(6)
                .map(|l| l.chars().take(300).collect())
                .collect(),
            art: ok.then(|| artifacts::collect(&out, target)),
            exe,
            log,
        }
    }

    /// Apply `n` random edits to the fixture's sources; returns the unified diff.
    fn edit(&self, src: &Path, rng: &mut StdRng, n: usize) -> String {
        let mut diff = String::new();
        for k in 0..n {
            let mut paths: Vec<PathBuf> = walkdir::WalkDir::new(src)
                .into_iter()
                .filter_map(Result::ok)
                .map(|e| e.into_path())
                .filter(|p| p.extension().is_some_and(|x| x == "rs") && !p.strip_prefix(src).unwrap().components().any(|c| c.as_os_str() == "target"))
                .collect();
            paths.sort();
            if paths.is_empty() {
                break;
            }
            for _ in 0..20 {
                let path = &paths[rng.random_range(..paths.len())];
                let (name, f) = mutations::pick(rng);
                if (name == "int_literal" || name == "str_literal") && path.file_name().is_some_and(|n| n == "build.rs") {
                    continue; // stale OUT_DIR files, or a build script that loops
                }
                let Ok(old) = std::fs::read_to_string(path) else { continue };
                match f(&old, rng, k) {
                    Some(new) if new != old => {
                        let _ = std::fs::write(path, &new);
                        let rel = path.strip_prefix(src).unwrap().display().to_string();
                        let d = similar::TextDiff::from_lines(&old, &new);
                        diff += &d.unified_diff().header(&format!("a/{rel}"), &format!("b/{rel}")).to_string();
                        break;
                    }
                    _ => {}
                }
            }
        }
        diff
    }

    fn run_exe(exe: Option<&str>) -> Value {
        let Some(exe) = exe else { return Value::Null };
        match run_command(Command::new(exe), Duration::from_secs(30)) {
            Ok(f) if f.exit == Exit::Timeout => serde_json::json!(["timeout", ""]),
            Ok(f) => {
                let code = match f.exit {
                    Exit::Code(c) => c,
                    Exit::Signal(s) => -s,
                    Exit::Timeout => unreachable!(),
                };
                serde_json::json!([code, tail(&f.stdout_text(), 2000)])
            }
            Err(_) => Value::Null,
        }
    }

    fn write_finding(&self, i: usize, res: &RowResult, findings: &[String], logs: &[(&str, &str)]) {
        let d = self.work.join("findings").join(format!("r{i}"));
        let _ = std::fs::create_dir_all(&d);
        let mut r = res.clone();
        r.findings = findings.to_vec();
        let _ = std::fs::write(d.join("row.json"), to_json_indent(&r, 1));
        if let Some(diff) = res.diff.as_ref().filter(|d| !d.is_empty()) {
            let _ = std::fs::write(d.join("edit.diff"), diff);
        }
        for (name, log) in logs {
            let _ = std::fs::write(d.join(name), tail(log, 20000));
        }
    }

    fn walk(&self, i: usize, row: &[(String, String)]) {
        if self.work.join("PAUSED").exists() || self.work.join("STOP").exists() {
            return;
        }
        let home = self.work.join(format!("r{i}"));
        let _ = std::fs::remove_dir_all(&home);
        let (src, target, inc_target) = (home.join("src"), home.join("target"), home.join("target-inc"));
        if let Err(e) = copy_fixture(&self.fixture, &src) {
            eprintln!("row {i}: cannot copy the fixture: {e}");
            return;
        }
        let (a, b) = (self.flags(row, "A"), self.flags(row, "B"));
        let mut res = RowResult {
            row: i,
            a: a[1..].to_vec(),
            b: b[1..].to_vec(),
            a_ok: false,
            diff: None,
            inc_ok: None,
            clean_ok: None,
            reuse: None,
            untracked: None,
            error: String::new(),
            errors: vec![],
            findings: vec![],
            rustc: String::new(),
        };
        let first = self.build(&src, &target, &a);
        res.a_ok = first.ok;
        let mut findings: Vec<String> = Vec::new();
        if first.ice {
            findings.push("ICE in clean A".into());
        }
        if first.ok {
            let mut rng = StdRng::seed_from_u64(self.args.seed.wrapping_mul(1_000_003).wrapping_add(i as u64));
            res.diff = Some(self.edit(&src, &mut rng, self.args.edits));
            let inc = self.build(&src, &target, &b);
            let _ = std::fs::rename(&target, &inc_target);
            let clean = self.build(&src, &target, &b);
            res.inc_ok = Some(inc.ok);
            res.clean_ok = Some(clean.ok);
            res.reuse = Some(inc.reuse.clone());
            res.untracked = Some(inc.untracked.clone());
            if inc.ice {
                findings.push("ICE in rebuild B".into());
            }
            if clean.ice {
                findings.push("ICE in clean B".into());
            }
            if inc.ok != clean.ok {
                findings.push(format!(
                    "split: rebuild {}, clean {}",
                    if inc.ok { "ok" } else { "failed" },
                    if clean.ok { "ok" } else { "failed" }
                ));
            }
            if inc.ok && clean.ok {
                let (ia, ca) = (inc.art.as_ref().unwrap(), clean.art.as_ref().unwrap());
                let mut diff = artifacts::compare(ia, ca);
                if matches!(flag_model::get(row, "B_Csplit_debuginfo"), Some("packed" | "unpacked")) {
                    // Objects and binary name .dwo files by session (DW_AT_GNU_dwo_name, and
                    // the dwo_id hashed from it), so two clean builds differ too.
                    diff.remove("rlib");
                    diff.remove("exe");
                }
                let inc_exe = inc.exe.as_ref().map(|e| e.replace(&*target.to_string_lossy(), &inc_target.to_string_lossy()));
                let ra = Self::run_exe(inc_exe.as_deref());
                let rb = Self::run_exe(clean.exe.as_deref());
                if ra != rb {
                    findings.push(format!("run: {} vs {}", py_value(&ra), py_value(&rb)));
                }
                if !diff.is_empty() {
                    // Clean builds may differ among themselves (P5), sometimes only one time in
                    // five: build clean again up to --p5-builds times before calling it reuse.
                    let mut p5 = BTreeMap::new();
                    for _ in 0..self.args.p5_builds {
                        let _ = std::fs::remove_dir_all(&target);
                        let again = self.build(&src, &target, &b);
                        if let Some(art) = &again.art {
                            p5 = artifacts::compare(ca, art);
                        }
                        if !p5.is_empty() || !again.ok {
                            break;
                        }
                    }
                    let kind = if p5.is_empty() { "" } else { "P5 " };
                    for (k, v) in &diff {
                        findings.push(format!("{kind}{k}: {}", py_list(&v[..v.len().min(5)])));
                    }
                }
            }
            res.error = if inc.error.is_empty() { clean.error.clone() } else { inc.error.clone() };
            res.errors = if inc.errors.is_empty() { clean.errors.clone() } else { inc.errors.clone() };
            if !findings.is_empty() {
                self.write_finding(i, &res, &findings, &[("inc.log", &inc.log), ("clean.log", &clean.log)]);
            }
        } else {
            res.error = first.error.clone();
            res.errors = first.errors.clone();
            if !findings.is_empty() {
                self.write_finding(i, &res, &findings, &[("a.log", &first.log)]);
            }
        }
        res.findings = findings.clone();
        res.rustc = self.args.rustc.clone();
        let new: Vec<&String> = findings.iter().filter(|f| !f.starts_with("known")).collect();
        if !new.is_empty() && self.args.pause_on_finding {
            #[derive(Serialize)]
            struct Paused<'a> {
                row: usize,
                findings: Vec<&'a String>,
            }
            let _ = std::fs::write(self.work.join("PAUSED"), to_json_indent(&Paused { row: i, findings: new }, 1));
        }
        let _ = std::fs::remove_dir_all(&home);
        {
            let _lock = self.results.lock().unwrap();
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.work.join("results.jsonl")) {
                let _ = writeln!(f, "{}", serde_json::to_string(&res).unwrap());
            }
        }
        let mut line = format!("row {i}: A {}", if res.a_ok { "ok" } else { "failed" });
        if res.a_ok {
            line += &format!(", rebuild {}", if res.inc_ok == Some(true) { "ok" } else { "failed" });
        }
        if !findings.is_empty() {
            line += &format!("; {}", py_list(&findings));
        }
        if !res.error.is_empty() {
            line += &format!("; {}", res.error.chars().take(100).collect::<String>());
        }
        println!("{line}");
    }
}

/// The last result of each row, from <work>/results.jsonl.
fn latest(work: &Path) -> BTreeMap<usize, RowResult> {
    let mut out = BTreeMap::new();
    for line in std::fs::read_to_string(work.join("results.jsonl")).unwrap_or_default().lines() {
        if let Ok(r) = serde_json::from_str::<RowResult>(line) {
            out.insert(r.row, r);
        }
    }
    out
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let work = std::fs::canonicalize(&args.work)?;
    let fixture = std::fs::canonicalize(&args.fixture)?;
    let _ = std::fs::remove_file(work.join("PAUSED"));
    let rows = flag_model::table(&args.table)?;
    let idx = slice(&args.rows, rows.len())?;
    // Resume: rows with a result are done, except, with --recheck, those with findings, which
    // run first.
    let done = latest(&work);
    let again: Vec<usize> =
        if args.recheck { idx.iter().copied().filter(|i| done.get(i).is_some_and(|r| !r.findings.is_empty())).collect() } else { vec![] };
    let mut todo = again.clone();
    todo.extend(idx.iter().copied().filter(|i| !done.contains_key(i)));
    if !again.is_empty() {
        println!("rechecking rows {:?}", again);
    }
    let walk = Walk { opts: flag_model::options(&args.flags)?, fixture, work: work.clone(), results: Mutex::new(()), args: &args };
    // Rows in table order, a worker taking the next one when it is free.
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..args.workers.max(1) {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::SeqCst);
                    let Some(&i) = todo.get(k) else { break };
                    walk.walk(i, &rows[i]);
                }
            });
        }
    });
    let results: Vec<RowResult> = latest(&work).into_values().collect();
    let mut summary = format!(
        "{{\"rows\": {}, \"done\": {}, \"A ok\": {}, \"compared\": {}, \"findings\": {}",
        rows.len(),
        results.len(),
        results.iter().filter(|r| r.a_ok).count(),
        results.iter().filter(|r| r.inc_ok == Some(true) && r.clean_ok == Some(true)).count(),
        results.iter().filter(|r| !r.findings.is_empty()).count()
    );
    if let Ok(p) = std::fs::read_to_string(work.join("PAUSED")) {
        if let Ok(v) = serde_json::from_str::<Value>(&p) {
            let f: Vec<String> = v["findings"].as_array().into_iter().flatten().map(|x| x.to_string()).collect();
            summary += &format!(", \"paused\": {{\"row\": {}, \"findings\": [{}]}}", v["row"], f.join(", "));
        }
    }
    println!("{summary}}}");
    Ok(ExitCode::SUCCESS)
}

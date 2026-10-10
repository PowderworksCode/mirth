//! Incremental rebuilds of rustc's UI tests, most of which fail to compile on purpose: the
//! fuzzer's edits applied to each test file, each rebuild compared with a clean build of the same
//! source, so that error reporting and recovery are exercised under incremental compilation.
//!
//! For each test in the list (ui-coverage's pick), compiled the way its `//@` headers say: build
//! it incrementally, then repeatedly apply a random edit (`mutations`) and rebuild it
//! incrementally, build the edited file again with a fresh incremental directory (same file, same
//! working directory), and compare:
//!
//!   status  both succeed, both fail, or both crash
//!   diag    the diagnostics, with paths and the incremental directory taken out
//!   output  the .rmeta and .rlib (normalized as `artifacts` does) when both succeed
//!   ice     both crash or neither does
//!
//! A clean build is made again before a difference counts (nondeterminism: such findings are
//! marked P5). Findings go to <work>/findings/<test>-<n>/ with the source, both outputs and the
//! edit history.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use mirth_lab::artifacts::{normalized_rlib, sha256};
use mirth_lab::rustc::{Exit, is_ice, run_command};
use mirth_lab::uitest::{self, Kind};
use mirth_lab::mutations;
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    #[arg(long)]
    tests: PathBuf,
    /// The tests to fuzz: a JSON list of paths, or of objects with a "test" path.
    #[arg(long)]
    list: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value_t = 20)]
    edits: usize,
    #[arg(long, default_value_t = 8)]
    jobs: usize,
    /// Extra rustc options for every build.
    #[arg(long, default_value = "", allow_hyphen_values = true)]
    flags: String,
    #[arg(long)]
    pause_on_finding: bool,
}

/// Tests that hit bugs already in docs/hunt.md every time: finding 17.
const KNOWN: &[&str] = &["unleash-the-miri-inside-of-you"];

#[derive(PartialEq)]
enum Digest {
    File(String),
    Rlib(BTreeMap<String, String>),
}

struct Build {
    code: i32,
    ice: bool,
    diag: Vec<String>,
    files: BTreeMap<String, Digest>,
    stderr: String,
}

static COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(\d+\)").unwrap());
static NON_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\W").unwrap());

struct Ctx<'a> {
    args: &'a Args,
    work: PathBuf,
}

impl Ctx<'_> {
    /// Compile `source` (a file in `dir`) with outputs in `dir/out`.
    fn build(&self, dir: &Path, source: &Path, flags: &[String], edition: &str, kind: Option<Kind>, incremental: Option<&Path>) -> Build {
        let out = dir.join("out");
        let _ = std::fs::remove_dir_all(&out);
        let _ = std::fs::create_dir_all(&out);
        let emit = if Kind::is_check(kind) { "--emit=metadata" } else { "--emit=link,metadata" };
        let mut cmd = Command::new(&self.args.rustc);
        cmd.arg(source.file_name().unwrap())
            .args(["--edition", edition, emit, "--out-dir", "out", "-Zunstable-options", "-Ainternal_features", "-Aincomplete_features"])
            .arg("--error-format=short");
        // Before the test's own flags, which may end with an option expecting a value.
        if let Some(i) = incremental {
            cmd.arg(format!("-Cincremental={}", i.display()));
        }
        cmd.args(flags).args(self.args.flags.split_whitespace());
        cmd.current_dir(dir).env("RUSTC_BOOTSTRAP", "1").env("RUST_BACKTRACE", "0");
        let (code, mut err) = match run_command(cmd, Duration::from_secs(300)) {
            Ok(f) => match f.exit {
                Exit::Code(c) => (c, f.stderr_text()),
                Exit::Signal(s) => (-s, f.stderr_text()),
                Exit::Timeout => (-1, "timeout".to_owned()),
            },
            Err(e) => (-1, e.to_string()),
        };
        let ice = is_ice(&err);
        if let Some(i) = incremental {
            err = err.replace(&i.display().to_string(), "<incremental>");
        }
        let diag: BTreeSet<String> = err
            .lines()
            .filter(|l| !l.is_empty() && !["note: ", "  ", "query stack", "#"].iter().any(|p| l.starts_with(p)))
            .map(|l| COUNT.replace_all(l, "(…)").into_owned())
            .collect();
        let mut files = BTreeMap::new();
        for e in std::fs::read_dir(&out).into_iter().flatten().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            match p.extension().and_then(|x| x.to_str()) {
                Some("rmeta") => {
                    files.insert(name, Digest::File(sha256(&std::fs::read(&p).unwrap_or_default())));
                }
                Some("rlib") => {
                    files.insert(name, Digest::Rlib(normalized_rlib(&p)));
                }
                _ => {}
            }
        }
        Build { code, ice, diag: diag.into_iter().collect(), files, stderr: err }
    }

    fn fuzz_one(&self, test: &str) -> anyhow::Result<String> {
        if self.work.join("PAUSED").exists() {
            return Ok("not run".into());
        }
        let path = self.args.tests.join(test);
        let text = String::from_utf8_lossy(&std::fs::read(&path)?).into_owned();
        let (flags, edition, kind, _) = uitest::headers(&text);
        let edition = edition.as_deref().unwrap_or("2015");
        if KNOWN.iter().any(|k| text.contains(k)) {
            return Ok("skipped (known)".into());
        }
        let name = format!("{}-{}", NON_WORD.replace_all(test, "_"), &sha256(test.as_bytes())[..8]);
        let home = self.work.join("w").join(&name);
        let _ = std::fs::remove_dir_all(&home);
        // The clean build uses the same directory and file, with a fresh incremental directory,
        // so that nothing but incremental state tells the two builds apart.
        let dir = home.join("src");
        std::fs::create_dir_all(&dir)?;
        let file_name = path.file_name().unwrap();
        let src = dir.join(file_name);
        std::fs::write(&src, &text)?;
        let (incr, incr_clean) = (home.join("incr"), home.join("incr-clean"));
        let mut rng = mutations::seeded(test);
        let mut history: Vec<serde_json::Value> = Vec::new();
        self.build(&dir, &src, &flags, edition, kind, Some(&incr));
        let mut found_any = 0;
        for n in 0..self.args.edits {
            let old = std::fs::read_to_string(&src)?;
            let (edit, f) = mutations::pick(&mut rng);
            let Some(new) = f(&old, &mut rng, n).filter(|new| *new != old) else { continue };
            std::fs::write(&src, &new)?;
            let diff = similar::TextDiff::from_lines(&old, &new).unified_diff().header("a", "b").to_string();
            history.push(serde_json::json!({"edit": edit, "diff": diff}));
            let inc = self.build(&dir, &src, &flags, edition, kind, Some(&incr));
            let _ = std::fs::remove_dir_all(&incr_clean);
            let clean = self.build(&dir, &src, &flags, edition, kind, Some(&incr_clean));
            let mut found = compare(&inc, &clean);
            if !found.is_empty() && !found.iter().any(|f| f.starts_with("ICE")) {
                let _ = std::fs::remove_dir_all(&incr_clean);
                let again = self.build(&dir, &src, &flags, edition, kind, Some(&incr_clean));
                if !compare(&clean, &again).is_empty() {
                    found = found.into_iter().map(|f| format!("P5 {f}")).collect();
                }
            }
            if found.is_empty() {
                continue;
            }
            found_any += 1;
            let d = self.work.join("findings").join(format!("{name}-{n}"));
            std::fs::create_dir_all(&d)?;
            std::fs::write(d.join(file_name), &new)?;
            let detail = serde_json::json!({"test": test, "found": found, "flags": flags, "edition": edition,
                                            "kind": kind, "history": history});
            std::fs::write(d.join("finding.json"), to_json(&detail))?;
            std::fs::write(d.join("inc.stderr"), &inc.stderr)?;
            std::fs::write(d.join("clean.stderr"), &clean.stderr)?;
            if self.args.pause_on_finding && !found.iter().all(|f| f.starts_with("P5") || f.starts_with("known")) {
                std::fs::write(self.work.join("PAUSED"), to_json(&serde_json::json!({"test": test, "edit": n, "found": found})))?;
                break;
            }
        }
        let _ = std::fs::remove_dir_all(&home);
        Ok(format!("{} edits, {found_any} findings", history.len()))
    }
}

/// JSON indented by one space, as the findings have always been written.
fn to_json(v: &impl Serialize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut out, serde_json::ser::PrettyFormatter::with_indent(b" "));
    v.serialize(&mut ser).expect("serializable");
    out
}

fn compare(inc: &Build, clean: &Build) -> Vec<String> {
    let mut found = Vec::new();
    // Some tests crash the compiler on purpose; only a crash on one side counts.
    if inc.ice != clean.ice {
        found.push(format!("ICE {} only", if inc.ice { "incremental" } else { "clean" }));
    }
    if (inc.code == 0) != (clean.code == 0) {
        found.push(format!("status: incremental {}, clean {}", inc.code, clean.code));
    }
    let message = |l: &str| -> String {
        let l = l.split_once(": error").map_or(l, |x| x.1);
        l.split_once(": warning").map_or(l, |x| x.1).to_owned()
    };
    let summary = |l: &str| l.starts_with("error: aborting") || (l.starts_with("warning:") && l.contains("emitted"));
    if inc.diag != clean.diag {
        let clean_set: HashSet<&String> = clean.diag.iter().collect();
        let inc_messages: HashSet<String> = inc.diag.iter().map(|d| message(d)).collect();
        let fewer = inc.code != 0
            && clean.code != 0
            && inc.diag.iter().filter(|d| !summary(d)).all(|d| clean_set.contains(d))
            && clean.diag.iter().filter(|d| !inc.diag.contains(d)).all(|d| inc_messages.contains(&message(d)) || summary(d));
        if fewer {
            found.push("known diag (finding 18): the rebuild stopped at a fatal error sooner".into());
        } else {
            let only_inc: Vec<&String> = inc.diag.iter().filter(|d| !clean.diag.contains(d)).take(3).collect();
            let only_clean: Vec<&String> = clean.diag.iter().filter(|d| !inc.diag.contains(d)).take(3).collect();
            found.push(format!("diag: incremental only {only_inc:?}; clean only {only_clean:?}"));
        }
    }
    if inc.code == 0 && clean.code == 0 && inc.files != clean.files {
        let names: BTreeSet<&String> = inc.files.keys().chain(clean.files.keys()).collect();
        let differ: Vec<&str> = names.into_iter().filter(|k| inc.files.get(*k) != clean.files.get(*k)).map(String::as_str).collect();
        found.push(format!("output: {}", differ.join(", ")));
    }
    found
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let work = std::fs::canonicalize(&args.work)?;
    let _ = std::fs::remove_file(work.join("PAUSED"));
    let ctx = Ctx { args: &args, work: work.clone() };
    // A compiler without its standard library fails every test the same way: stop instead.
    let probe_dir = work.join("probe");
    std::fs::create_dir_all(&probe_dir)?;
    std::fs::write(probe_dir.join("probe.rs"), "fn main() {}\n")?;
    let probe = ctx.build(&probe_dir, &probe_dir.join("probe.rs"), &[], "2021", Some(Kind::BuildPass), None);
    if probe.code != 0 {
        eprintln!("{} cannot build an empty program:\n{}", args.rustc.display(), probe.stderr);
        return Ok(ExitCode::from(1));
    }
    let picked: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(&args.list)?)?;
    let tests: Vec<String> = picked
        .iter()
        .filter_map(|t| t.get("test").unwrap_or(t).as_str().map(str::to_owned))
        .collect();
    let done_path = work.join("done.txt");
    let done: HashSet<String> = std::fs::read_to_string(&done_path).unwrap_or_default().split_whitespace().map(str::to_owned).collect();
    let log = Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(&done_path)?);
    let todo: Vec<&String> = tests.iter().filter(|t| !done.contains(*t)).collect();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;
    pool.install(|| {
        todo.par_iter().for_each(|test| {
            // A broken test or harness case must not stop the run.
            let result = ctx.fuzz_one(test).unwrap_or_else(|e| format!("harness error: {e:#}").chars().take(300).collect());
            println!("{test}: {result}");
            if !result.starts_with("not run") && !work.join("PAUSED").exists() {
                let mut log = log.lock().unwrap();
                let _ = writeln!(log, "{test}");
                let _ = log.flush();
            }
        })
    });
    Ok(ExitCode::SUCCESS)
}

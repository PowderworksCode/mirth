//! Which compiler functions each of rustc's UI tests reaches that a baseline (the fixture's
//! builds) does not, with a coverage-instrumented rustc (rustc/coverage.toml); then a small set
//! of tests that reaches the most of them.
//!
//! `run` compiles each test file the way its `//@` headers say, as far as one rustc call can:
//! `compile-flags`, `edition`, the first of `revisions` (as `--cfg` with its own flags), metadata
//! only for tests that do not build (check-pass, and tests expected to fail before codegen), a
//! full build otherwise. Tests that need auxiliary crates, proc macros, another target or
//! `minicore` are skipped. Writes <out>/tests.jsonl: per test, whether it compiled and the
//! indices (into <out>/functions.json) of the functions it reached beyond the baseline.
//!
//! `pick` chooses tests greedily, each adding the most functions not yet reached, and writes
//! <out>/picked.json.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use mirth_lab::coverage::{self, to_json_indent, to_json_line};
use mirth_lab::rustc::{is_ice, run_command, Exit};
use mirth_lab::uitest::{self, Kind};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Subcommand, Debug)]
enum Cmd {
    /// Compile each UI test; record the functions it reaches beyond the baseline.
    Run(RunArgs),
    /// Choose the tests that together reach the most.
    Pick(PickArgs),
}

#[derive(clap::Args, Debug)]
struct RunArgs {
    #[arg(long)]
    rustc: PathBuf,
    #[arg(long)]
    tests: PathBuf,
    #[arg(long)]
    sites: PathBuf,
    /// Log directories (or directories of them) of the baseline.
    #[arg(long)]
    baseline: Vec<PathBuf>,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = 6)]
    jobs: usize,
    #[arg(long, default_value_t = 0)]
    limit: usize,
}

#[derive(clap::Args, Debug)]
struct PickArgs {
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = 200)]
    count: usize,
}

static SKIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^//@\s*(aux-build|aux-crate|aux-bin|proc-macro|add-minicore|needs-llvm-components|only-|ignore-x86_64|ignore-linux|needs-sanitizer|needs-profiler|needs-asm-support|known-bug)").unwrap()
});
/// `only-<x>` directives this host satisfies.
const HOST_ONLY: &[&str] = &["x86_64", "linux", "unix", "64bit"];

fn skipped(text: &str) -> bool {
    SKIP.captures_iter(text).any(|c| {
        let m = c.get(1).unwrap();
        m.as_str() != "only-" || !HOST_ONLY.iter().any(|h| text[m.end()..].starts_with(h))
    })
}

#[derive(Serialize)]
#[serde(untagged)]
enum Outcome {
    Ran { test: String, status: String, kind: Option<Kind>, error: String, new: Vec<usize> },
    Skipped { test: String, status: String },
}

#[derive(Serialize)]
struct Functions<'a> {
    functions: Vec<[&'a str; 2]>,
    baseline: Vec<usize>,
}

fn run_one(a: &RunArgs, path: &Path, index: &BTreeMap<String, usize>, baseline: &HashSet<String>) -> Outcome {
    let text = String::from_utf8_lossy(&std::fs::read(path).unwrap_or_default()).into_owned();
    let rel = path.strip_prefix(&a.tests).unwrap_or(path).to_string_lossy().into_owned();
    if skipped(&text) {
        return Outcome::Skipped { test: rel, status: "skipped".into() };
    }
    let (flags, edition, kind, _) = uitest::headers(&text);
    let d = tempfile::tempdir_in(a.out.join("scratch")).expect("scratch directory");
    let emit = if Kind::is_check(kind) { "--emit=metadata" } else { "--emit=link" };
    let logs = d.path().join("logs");
    let mut cmd = Command::new(&a.rustc);
    cmd.arg(path)
        .args(["--edition", edition.as_deref().unwrap_or("2015"), emit, "--out-dir"])
        .arg(d.path())
        .args(["-Zunstable-options", "-Ainternal_features"])
        .args(&flags)
        .env("MIRTH_OUT", &logs)
        .env("RUSTC_BOOTSTRAP", "1")
        .current_dir(d.path());
    let (status, error) = match run_command(cmd, Duration::from_secs(120)) {
        Ok(f) if f.exit == Exit::Timeout => ("timeout".to_owned(), String::new()),
        Ok(f) => {
            let stderr = f.stderr_text();
            let status = if f.success() { "ok" } else if is_ice(&stderr) { "ice" } else { "error" };
            let first: String = stderr.lines().find(|l| l.starts_with("error")).unwrap_or("").chars().take(160).collect();
            (status.to_owned(), first)
        }
        Err(e) => ("error".to_owned(), e.to_string()),
    };
    let (hit, _) = coverage::hits(&logs);
    let mut new: Vec<usize> = hit.iter().filter(|s| !baseline.contains(*s)).filter_map(|s| index.get(s).copied()).collect();
    new.sort();
    Outcome::Ran { test: rel, status, kind, error, new }
}

fn run_tests(a: RunArgs) -> anyhow::Result<()> {
    let mut functions: Vec<(String, String, String)> = coverage::functions(&a.sites).into_iter().map(|f| (f.site, f.path, f.span)).collect();
    functions.sort();
    let index: BTreeMap<String, usize> = functions.iter().enumerate().map(|(i, f)| (f.0.clone(), i)).collect();
    let mut baseline: HashSet<String> = HashSet::new();
    for b in &a.baseline {
        let mut dirs = vec![b.clone()];
        if let Ok(entries) = std::fs::read_dir(b) {
            dirs.extend(entries.flatten().map(|e| e.path()).filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))));
        }
        for d in dirs.iter().filter(|d| d.is_dir()) {
            baseline.extend(coverage::hits(d).0);
        }
    }
    std::fs::create_dir_all(a.out.join("scratch"))?;
    let mut base_idx: Vec<usize> = baseline.iter().filter_map(|s| index.get(s).copied()).collect();
    base_idx.sort();
    let info = Functions { functions: functions.iter().map(|f| [f.1.as_str(), f.2.as_str()]).collect(), baseline: base_idx.clone() };
    std::fs::write(a.out.join("functions.json"), to_json_line(&info))?;
    let results = a.out.join("tests.jsonl");
    let done: HashSet<String> = std::fs::read_to_string(&results)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v["test"].as_str().map(str::to_owned))
        .collect();
    let mut tests: Vec<PathBuf> = WalkDir::new(&a.tests)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs") && p.is_file())
        .filter(|p| {
            let rel = p.strip_prefix(&a.tests).unwrap_or(p);
            !rel.components().any(|c| c.as_os_str() == "auxiliary") && !done.contains(&*rel.to_string_lossy())
        })
        .collect();
    tests.sort();
    if a.limit > 0 {
        tests.truncate(a.limit);
    }
    println!("{} functions, {} in the baseline; {} tests to run", functions.len(), base_idx.len(), tests.len());
    let out = Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(&results)?);
    let n = AtomicUsize::new(0);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(a.jobs).build()?;
    pool.install(|| {
        tests.par_iter().for_each(|t| {
            let res = run_one(&a, t, &index, &baseline);
            let mut f = out.lock().unwrap();
            let _ = writeln!(f, "{}", to_json_line(&res));
            let k = n.fetch_add(1, Ordering::Relaxed);
            if k % 500 == 0 {
                let _ = f.flush();
                println!("{k} tests");
            }
        })
    });
    let _ = std::fs::remove_dir_all(a.out.join("scratch"));
    Ok(())
}

/// What `pick` reads of a line of tests.jsonl.
#[derive(Deserialize)]
struct Line {
    test: String,
    status: String,
    #[serde(default)]
    new: Vec<usize>,
}

#[derive(Deserialize)]
struct Info {
    functions: Vec<serde_json::Value>,
    baseline: Vec<usize>,
}

#[derive(Serialize)]
struct Picked {
    test: String,
    status: String,
    adds: usize,
    total: usize,
}

fn pick(a: PickArgs) -> anyhow::Result<()> {
    let info: Info = serde_json::from_str(&std::fs::read_to_string(a.out.join("functions.json"))?)?;
    // In file order: a tie goes to the test listed first, as Python's max does.
    let mut sets: Vec<(String, BTreeSet<usize>)> = Vec::new();
    let mut status: BTreeMap<String, String> = BTreeMap::new();
    let mut reached = BTreeSet::new();
    for line in std::fs::read_to_string(a.out.join("tests.jsonl"))?.lines() {
        let Ok(Line { test, status: s, new }) = serde_json::from_str(line) else { continue };
        if new.is_empty() {
            continue;
        }
        reached.extend(new.iter().copied());
        let set: BTreeSet<usize> = new.into_iter().collect();
        match sets.iter_mut().find(|(t, _)| *t == test) {
            Some(entry) => entry.1 = set,
            None => sets.push((test.clone(), set)),
        }
        status.insert(test, s);
    }
    let mut covered: BTreeSet<usize> = BTreeSet::new();
    let mut picked = Vec::new();
    while picked.len() < a.count && !sets.is_empty() {
        let gains: Vec<usize> = sets.iter().map(|(_, s)| s.difference(&covered).count()).collect();
        let best = (0..sets.len()).fold(0, |b, i| if gains[i] > gains[b] { i } else { b });
        if gains[best] == 0 {
            break;
        }
        let (test, set) = sets.remove(best);
        covered.extend(set);
        picked.push(Picked { status: status[&test].clone(), test, adds: gains[best], total: covered.len() });
    }
    let total = info.functions.len();
    let base = info.baseline.len();
    println!(
        "baseline {base} of {total} functions ({:.1}%); all tests reach {} more; {} picked tests reach {} more ({:.1}% in all)",
        100.0 * base as f64 / total as f64,
        reached.len(),
        picked.len(),
        covered.len(),
        100.0 * (base + covered.len()) as f64 / total as f64
    );
    for t in picked.iter().take(40) {
        println!("  +{:5} {:6}  {:7} {}", t.adds, t.total, t.status, t.test);
    }
    std::fs::write(a.out.join("picked.json"), to_json_indent(&picked, 1))?;
    Ok(())
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    match args.cmd {
        Cmd::Run(a) => run_tests(a)?,
        Cmd::Pick(a) => pick(a)?,
    }
    Ok(ExitCode::SUCCESS)
}

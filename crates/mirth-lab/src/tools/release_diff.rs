//! Release-to-release: code that one toolchain accepts the next must accept too, in comparable
//! time.
//!
//! `cargo check --locked --offline --workspace` of each repository in a corpus of real crates
//! with an older and a newer toolchain (dependencies fetched first), each in its own target
//! directory, removed afterwards. Findings: a regression (old passes, new fails), an ICE, or the
//! new toolchain taking more than --slower times as long. A regression whose failing crate
//! enables unstable features (`#![feature]`, often only when it detects a nightly) is noted, not
//! reported.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use mirth_lab::rustc::{Exit, error_codes, is_ice, run_command};
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// A directory of repositories (each with a Cargo.toml).
    #[arg(long)]
    corpus: PathBuf,
    #[arg(long)]
    old: String,
    #[arg(long)]
    new: String,
    #[arg(long)]
    work: PathBuf,
    #[arg(long)]
    only: Option<String>,
    #[arg(long, default_value_t = 2)]
    jobs: usize,
    #[arg(long, default_value_t = 1.5)]
    slower: f64,
    #[arg(long, default_value_t = 1800)]
    timeout: u64,
}

static ERROR_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\berror(\[E\d+\])?:").unwrap());
static CRATE_ROOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(/\S*?/registry/src/[^/]+/[^/]+|/\S+?)/src/").unwrap());
static FEATURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#!\[(cfg_attr\([^\]]*)?feature\(").unwrap());

#[derive(Serialize)]
struct Check {
    exit: Exit,
    seconds: f64,
    ice: bool,
    codes: Vec<String>,
    first: String,
    tail: String,
}

#[derive(Serialize)]
struct Rec {
    repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    old: Option<Check>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new: Option<Check>,
    found: Vec<String>,
    notes: Vec<String>,
}

fn check(args: &Args, repo: &Path, toolchain: &str) -> Check {
    let name = repo.file_name().unwrap().to_string_lossy();
    let target = args.work.join("target").join(format!("{name}-{toolchain}"));
    let _ = std::fs::remove_dir_all(&target);
    let mut cmd = Command::new("cargo");
    cmd.arg(format!("+{toolchain}"))
        .args(["check", "--locked", "--offline", "--workspace", "--message-format=short"])
        .current_dir(repo)
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_INCREMENTAL", "0")
        .env("RUSTFLAGS", "--cap-lints=warn")
        .env_remove("RUSTC_WRAPPER");
    let start = Instant::now();
    let done = run_command(cmd, Duration::from_secs(args.timeout));
    let seconds = (start.elapsed().as_secs_f64() * 10.0).round() / 10.0;
    let _ = std::fs::remove_dir_all(&target);
    let (exit, err) = match done {
        Ok(d) => (d.exit.clone(), d.stderr_text()),
        Err(e) => (Exit::Code(-1), e.to_string()),
    };
    let first = err.lines().find(|l| ERROR_LINE.is_match(l)).unwrap_or("").chars().take(300).collect();
    let tail = if exit == Exit::Code(0) { String::new() } else { err.chars().rev().take(3000).collect::<String>().chars().rev().collect() };
    Check { exit, seconds, ice: is_ice(&err), codes: error_codes(&err), first, tail }
}

/// The crate a first error points into, if its source enables `#![feature(...)]`.
fn uses_unstable(first_error: &str) -> Option<String> {
    let root = PathBuf::from(&CRATE_ROOT.captures(first_error)?[1]);
    ["lib.rs", "main.rs"].iter().find_map(|f| {
        let text = std::fs::read_to_string(root.join("src").join(f)).ok()?;
        FEATURE.is_match(&text).then(|| root.file_name().unwrap().to_string_lossy().into_owned())
    })
}

fn one(args: &Args, repo: &Path) -> Rec {
    let name = repo.file_name().unwrap().to_string_lossy().into_owned();
    let mut fetch = Command::new("cargo");
    fetch.arg(format!("+{}", args.new)).args(["fetch", "--locked"]).current_dir(repo);
    match run_command(fetch, Duration::from_secs(1800)) {
        Ok(d) if d.success() => {}
        other => {
            let why = other.map(|d| d.stderr_text()).unwrap_or_else(|e| e.to_string());
            return Rec { repo: name, skip: Some(format!("fetch failed: {}", why.chars().rev().take(300).collect::<String>().chars().rev().collect::<String>())), old: None, new: None, found: vec![], notes: vec![] };
        }
    }
    let old = check(args, repo, &args.old);
    let new = check(args, repo, &args.new);
    let (mut found, mut notes) = (Vec::new(), Vec::new());
    let ok = |c: &Check| c.exit == Exit::Code(0);
    if ok(&old) && !ok(&new) {
        match uses_unstable(&new.first) {
            Some(krate) if !new.ice => notes.push(format!("regression in a crate using unstable features ({krate})")),
            _ => found.push(if new.ice { "ice".into() } else { "regression".into() }),
        }
    } else if !ok(&old) && ok(&new) {
        notes.push("fixed".into());
    } else if ok(&old) && ok(&new) && old.seconds > 5.0 && new.seconds > args.slower * old.seconds {
        found.push(format!("slower: {}s -> {}s", old.seconds, new.seconds));
    }
    if new.ice && !found.iter().any(|f| f == "ice") {
        found.push("ice".into());
    }
    Rec { repo: name, skip: None, old: Some(old), new: Some(new), found, notes }
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let mut repos: Vec<PathBuf> = std::fs::read_dir(&args.corpus)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").exists() && args.only.as_ref().is_none_or(|o| p.to_string_lossy().contains(o.as_str())))
        .collect();
    repos.sort();
    println!("{} repositories, {} -> {}", repos.len(), args.old, args.new);
    let out = std::sync::Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(args.work.join("results.jsonl"))?);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;
    pool.install(|| {
        repos.par_iter().for_each(|repo| {
            let rec = one(&args, repo);
            if let Ok(mut f) = out.lock() {
                use std::io::Write;
                let _ = writeln!(f, "{}", serde_json::to_string(&rec).unwrap_or_default());
            }
            let tag = rec.skip.clone().unwrap_or_else(|| if rec.found.is_empty() { rec.notes.join(", ") } else { rec.found.join(", ") });
            println!("{:45} {}", rec.repo, if tag.is_empty() { "same".into() } else { tag });
        })
    });
    Ok(ExitCode::SUCCESS)
}

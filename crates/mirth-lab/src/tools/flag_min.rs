//! Minimize the failures of a flag-walk run: for each distinct first error among rows whose
//! clean build with A failed, find the smallest set of the row's options that still gives the
//! same error (delta debugging, a clean build of the fixture per test).
//!
//! Prints one line per error: the minimal options. Writes <walk>/minimized.json.

use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::sync::LazyLock;

use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

use mirth_lab::coverage::to_json_indent;
use super::flag_model::{FLAG_BASE};
use super::flag_walk::copy_fixture;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    #[arg(long)]
    fixture: PathBuf,
    /// flag-walk's work directory.
    #[arg(long)]
    walk: PathBuf,
    #[arg(long, default_value_t = 1)]
    per_error: usize,
    #[arg(long, default_value_t = 4)]
    jobs: usize,
    #[arg(long, default_value = "nightly-2026-10-06")]
    toolchain: String,
    #[arg(long, default_value = "x86_64-unknown-linux-gnu")]
    target: String,
}

static VOLATILE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"_R\w+|/\S+|`[^`]*`|\b[0-9a-f]{16}\b").unwrap());
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

/// An error line with paths, symbols, hashes and quoted names removed.
pub fn signature(line: &str) -> String {
    let line = VOLATILE.replace_all(line, "…");
    DIGITS.replace_all(&line, "N").chars().take(120).collect()
}

fn first_error(log: &str) -> &str {
    for l in log.lines() {
        if (l.starts_with("error") || l.starts_with("rustc-LLVM ERROR") || l.starts_with("LLVM ERROR")) && !l.contains("could not compile") {
            return l;
        }
        if l.contains("panicked at") {
            return l;
        }
    }
    ""
}

#[derive(Deserialize)]
struct Row {
    #[serde(rename = "A")]
    a: Vec<String>,
    #[serde(rename = "A_ok")]
    a_ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    errors: Option<Vec<String>>,
}

#[derive(Serialize)]
struct Minimized {
    error: String,
    minimal: Option<Vec<String>>,
}

fn minimize(args: &Args, mut flags: Vec<String>, sig: &str) -> Option<Vec<String>> {
    let work = tempfile::tempdir_in(&args.walk).expect("scratch");
    let (src, target) = (work.path().join("s"), work.path().join("t"));
    copy_fixture(&args.fixture, &src).ok()?;
    let fails = |fl: &[String]| -> bool {
        let _ = std::fs::remove_dir_all(&target);
        let rustflags = std::iter::once(FLAG_BASE.to_owned()).chain(fl.iter().cloned()).collect::<Vec<_>>().join(" ");
        let r = Command::new("cargo")
            .arg(format!("+{}", args.toolchain))
            .args(["build", "--workspace", "--offline", "-j", "4", "--target", &args.target, "--target-dir"])
            .arg(&target)
            .current_dir(&src)
            .env("RUSTC", &args.rustc)
            .env("RUSTC_WRAPPER", "")
            .env("CARGO_INCREMENTAL", "1")
            .env("CARGO_TERM_COLOR", "never")
            .env("RUSTFLAGS", rustflags)
            .output();
        match r {
            Ok(o) => !o.status.success() && signature(first_error(&String::from_utf8_lossy(&o.stderr))) == sig,
            Err(_) => false,
        }
    };
    if !fails(&flags) {
        return None;
    }
    let mut n = 2;
    while flags.len() >= 2 {
        let chunk = (flags.len() / n).max(1);
        let mut reduced = false;
        let mut i = 0;
        while i < flags.len() {
            let rest: Vec<String> = flags[..i].iter().chain(flags[(i + chunk).min(flags.len())..].iter()).cloned().collect();
            if fails(&rest) {
                flags = rest;
                n = (n - 1).max(2);
                reduced = true;
                break;
            }
            i += chunk;
        }
        if !reduced {
            if chunk == 1 {
                break;
            }
            n = flags.len().min(n * 2);
        }
    }
    Some(flags)
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let text = std::fs::read_to_string(args.walk.join("results.jsonl"))?;
    // By signature, in first-seen order.
    let mut todo: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: Row = serde_json::from_str(line)?;
        if r.a_ok {
            continue;
        }
        let err = r.errors.as_ref().and_then(|e| e.first().cloned()).or(r.error.clone()).unwrap_or_default();
        let sig = signature(&err);
        let i = match todo.iter().position(|(s, _)| *s == sig) {
            Some(i) => i,
            None => {
                todo.push((sig, Vec::new()));
                todo.len() - 1
            }
        };
        if todo[i].1.len() < args.per_error {
            todo[i].1.push(r.a);
        }
    }
    let jobs: Vec<(&String, &Vec<String>)> = todo.iter().flat_map(|(s, fls)| fls.iter().map(move |f| (s, f))).collect();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;
    let out: Vec<Minimized> = pool.install(|| {
        jobs.par_iter().map(|(sig, fl)| Minimized { error: (*sig).clone(), minimal: minimize(&args, (*fl).clone(), sig) }).collect()
    });
    std::fs::write(args.walk.join("minimized.json"), to_json_indent(&out, 1))?;
    for o in &out {
        let m = match &o.minimal {
            Some(v) => format!("[{}]", v.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", ")),
            None => "None".into(),
        };
        println!("{m} -> {}", o.error);
    }
    Ok(ExitCode::SUCCESS)
}

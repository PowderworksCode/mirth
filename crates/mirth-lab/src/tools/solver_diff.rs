//! Solver differential: the old and new trait solvers, and NLL and Polonius, must agree.
//!
//! Compiles each standalone UI test four ways (old solver = `-Znext-solver=coherence`, the
//! default new solver, each with `-Zpolonius=next`) and compares with the old solver and NLL: a
//! crash, a timeout or a different verdict is a finding; both rejecting with different error
//! codes is a note. A program accepted only by a non-reference configuration is interpreted with
//! Miri under that configuration: undefined behavior means the other one accepted something
//! unsound. Tests that name a solver or Polonius in their headers are left out.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::miri::{Miri, MiriStatus};
use mirth_lab::rustc::{self, Compile, Status};
use mirth_lab::uitest::{self, Kind, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// The toolchain whose Miri interprets one-sided acceptances.
    #[arg(long, default_value = "nightly-2026-10-06")]
    miri_toolchain: String,
    #[command(flatten)]
    sweep: Sweep,
}

const CONFIGS: &[(&str, &[&str])] = &[
    ("old", &["-Znext-solver=coherence"]),
    ("next", &[]),
    ("old-polonius", &["-Znext-solver=coherence", "-Zpolonius=next"]),
    ("next-polonius", &["-Zpolonius=next"]),
];
static OWN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)next-solver|polonius|^//@\s*revisions:.*\bnext\b").unwrap());

#[derive(Serialize)]
struct Result1 {
    status: Status,
    codes: Vec<String>,
    stderr: String,
}

#[derive(Serialize)]
struct Finding {
    config: String,
    what: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    miri: Option<String>,
    stderr: String,
}

#[derive(Serialize)]
struct Rec {
    test: String,
    status: Vec<(String, Status)>,
    found: Vec<String>,
    notes: Vec<String>,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        self.found.clone()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

fn tail(s: &str, n: usize) -> String {
    let v: Vec<char> = s.chars().collect();
    v[v.len().saturating_sub(n)..].iter().collect()
}

fn check(args: &Args, miri: &Miri, test: &Test) -> Rec {
    let dir = driver::scratch_dir(&args.sweep);
    // Metadata for check tests; build and run tests get a full build (monomorphization errors).
    let emit = if Kind::is_check(test.kind) { "metadata" } else { "link" };
    let results: Vec<(&str, Result1)> = CONFIGS
        .iter()
        .map(|&(name, cfg)| {
            let c = Compile::new(&args.rustc, &test.path, &dir.path().join(name), &test.flags, test.edition())
                .extra(cfg.iter().copied())
                .emit(emit)
                .timeout(120)
                .run();
            (name, Result1 { status: c.status, codes: rustc::error_codes(&c.stderr), stderr: tail(&c.stderr, 2500) })
        })
        .collect();
    let reference = &results[0].1;
    let (mut found, mut notes) = (Vec::new(), Vec::new());
    for (name, r) in &results[1..] {
        if r.status == Status::Ice && reference.status != Status::Ice {
            found.push(Finding { config: name.to_string(), what: "ice".into(), miri: None, stderr: r.stderr.clone() });
        } else if r.status == Status::Timeout && reference.status != Status::Timeout {
            found.push(Finding { config: name.to_string(), what: "timeout".into(), miri: None, stderr: String::new() });
        } else if matches!((r.status, reference.status), (Status::Ok, Status::Error) | (Status::Error, Status::Ok)) {
            let accepted_by = if r.status == Status::Ok { name } else { "old" };
            let mut what = format!("verdict: old {:?}, {name} {:?}", reference.status, r.status).to_lowercase();
            let mut miri_status = None;
            if test.text.contains("fn main") {
                let cfg = CONFIGS.iter().find(|c| c.0 == accepted_by).map_or(&[][..], |c| c.1);
                let extra: Vec<String> = cfg.iter().map(|s| s.to_string()).collect();
                let m = miri.run(&test.path, &test.flags, test.edition(), &extra, 120, dir.path());
                if m.status == MiriStatus::Ub {
                    what.push_str(&format!("; Miri: UB under {accepted_by}"));
                }
                miri_status = Some(format!("{:?}", m.status));
            }
            let stderr = if r.status == Status::Error { r.stderr.clone() } else { reference.stderr.clone() };
            found.push(Finding { config: name.to_string(), what, miri: miri_status, stderr });
        } else if r.status == Status::Error && reference.status == Status::Error && r.codes != reference.codes {
            notes.push(format!("{name} codes {:?} vs {:?}", r.codes, reference.codes));
        }
    }
    let rec = Rec {
        test: test.rel.clone(),
        status: results.iter().map(|(n, r)| (n.to_string(), r.status)).collect(),
        found: found.iter().map(|f| format!("{}: {}", f.config, f.what)).collect(),
        notes,
    };
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &[], &serde_json::json!({ "found": found }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let miri = Miri::pinned(&args.miri_toolchain);
    let tests = uitest::tests(&args.sweep.tests, uitest::ALL, |t| OWN.is_match(&t.text));
    let tests = args.sweep.select(tests);
    println!("{} tests, configurations: old, next, old-polonius, next-polonius", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, &miri, t)))
}

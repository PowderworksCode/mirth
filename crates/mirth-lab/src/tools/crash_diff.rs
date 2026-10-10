//! Internal checks on: what rustc's own invariants say about every UI test.
//!
//! Compiles each standalone UI test with the compiler under test and with a second one built
//! from the same tree with debug assertions (and overflow checks), adding `-Zvalidate-mir`.
//! A crash, failed assertion or MIR validation error under the second that the first does not
//! have is a finding.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{Compile, Status};
use mirth_lab::uitest::{self, Kind, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The release compiler under test.
    #[arg(long)]
    rustc: PathBuf,
    /// The same compiler built with debug assertions.
    #[arg(long)]
    checked: PathBuf,
    #[arg(long, default_value = "-Zvalidate-mir")]
    extra: String,
    #[command(flatten)]
    sweep: Sweep,
}

static MESSAGE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [r"panicked at [^\n]*\n[^\n]*", r"internal compiler error: [^\n]*", r"broken MIR[^\n]*"]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect()
});
static COMPILER_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/\S+/compiler/").unwrap());

/// The first line saying what went wrong inside the compiler.
fn message(stderr: &str) -> String {
    MESSAGE
        .iter()
        .find_map(|re| re.find(stderr))
        .map(|m| COMPILER_PATH.replace_all(m.as_str(), "compiler/").chars().take(400).collect())
        .unwrap_or_default()
}

#[derive(Serialize)]
struct Rec {
    test: String,
    release: Status,
    checked: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    found: Vec<String>,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        self.found.clone()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

fn check(args: &Args, test: &Test) -> Rec {
    let dir = driver::scratch_dir(&args.sweep);
    let emit = if Kind::is_check(test.kind) { "metadata" } else { "link" };
    let a = Compile::new(&args.rustc, &test.path, &dir.path().join("release"), &test.flags, test.edition())
        .emit(emit)
        .run();
    let b = Compile::new(&args.checked, &test.path, &dir.path().join("checked"), &test.flags, test.edition())
        .extra(args.extra.split_whitespace())
        .emit(emit)
        .timeout(600)
        .run();
    let mut rec = Rec { test: test.rel.clone(), release: a.status, checked: b.status, note: None, found: Vec::new() };
    if b.status == Status::Ice && a.status != Status::Ice {
        let m = message(&b.stderr);
        rec.found.push(format!("only with internal checks: {}", m.chars().take(160).collect::<String>()));
        let stderr: String = b.stderr.chars().rev().take(4000).collect::<String>().chars().rev().collect();
        driver::write_finding(
            &args.sweep.work,
            test,
            &[],
            &serde_json::json!({ "extra": args.extra, "found": [{ "what": "only with internal checks", "message": m, "stderr": stderr }] }),
        );
    } else if b.status == Status::Ice && a.status == Status::Ice && message(&a.stderr) != message(&b.stderr) {
        rec.note = Some("both crash, differently".into());
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let tests = args.sweep.select(uitest::tests(&args.sweep.tests, uitest::ALL, |_| false));
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, t)))
}

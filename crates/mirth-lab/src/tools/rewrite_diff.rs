//! Equivalent rewrites: rewriting a program into an equivalent one must not change its verdict.
//!
//! Each standalone UI test is printed back unchanged (the `identity` rewrite: the baseline,
//! since printing drops comments and moves lines) and rewritten by each of mirth-rewrite's
//! rewrites, in process. Each version is compiled with lints capped (a full build when there is
//! a `fn main`), and a changed verdict is a finding; both rejecting with different error codes
//! is a note. Tests whose baseline differs from the original are left out (the printer cannot
//! represent them), as are tests that name files by relative path or have no `core`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{self, Compile, Status};
use mirth_lab::uitest::{self, Kind, Test};
use mirth_rewrite::{Outcome, REWRITES, rewrite};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// Comma-separated subset of rewrites.
    #[arg(long)]
    rewrites: Option<String>,
    #[command(flatten)]
    sweep: Sweep,
}

static NOT_MOVABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*;|include(_str|_bytes)?!|#\[path|#!\[no_core\]").unwrap());
/// Item order matters to textual macro scoping: no reordering where macros are defined.
static ORDER_MATTERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"macro_rules!|#\[macro_use\]|macro\s+\w+").unwrap());
/// Differences that are resource limits, or findings already recorded.
const NOISE: &[(&str, &str)] = &[
    ("consts/chained-constants-stackoverflow.rs", "reorder"), // 10,000 chained consts: query depth
    ("consts/interior-mut-const-via-union.rs", "generic-wrap"), // finding 25
    // recursion_limit = "6": evaluation order nests the query stack one level deeper
    ("traits/next-solver/overflow/dont-lower-depth-for-witness-and-rigid-opaque.rs", "reorder"),
    ("imports/ambiguous-9.rs", "reorder"), // finding 28
    ("imports/ambiguous-14.rs", "reorder"),
    ("imports/overwrite-different-ambig-2.rs", "reorder"),
];

#[derive(Serialize, Clone)]
struct Verdict {
    status: Status,
    codes: Vec<String>,
    stderr: String,
}

#[derive(Serialize)]
struct Rec {
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    applied: Vec<String>,
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

fn verdict(args: &Args, test: &Test, source: &std::path::Path, out: &std::path::Path) -> Verdict {
    // A full build whenever there is a program: generic-wrap moves errors to monomorphization.
    let emit = if test.text.contains("fn main") || !Kind::is_check(test.kind) { "link" } else { "metadata" };
    let c = Compile::new(&args.rustc, source, out, &test.flags, test.edition())
        .extra(["--cap-lints=warn"])
        .emit(emit)
        .timeout(120)
        .run();
    let stderr: String = c.stderr.chars().rev().take(2500).collect::<String>().chars().rev().collect();
    Verdict { status: c.status, codes: rustc::error_codes(&c.stderr), stderr }
}

fn check(args: &Args, names: &[&str], test: &Test) -> Rec {
    let mut rec = Rec { test: test.rel.clone(), skip: None, applied: Vec::new(), found: Vec::new(), notes: Vec::new() };
    if NOT_MOVABLE.is_match(&test.text) {
        rec.skip = Some("uses files by path or has no core".into());
        return rec;
    }
    let Outcome::Rewritten(identity) = rewrite("identity", &test.text) else {
        rec.skip = Some("does not parse".into());
        return rec;
    };
    let dir = driver::scratch_dir(&args.sweep);
    let base_src = dir.path().join("identity.rs");
    let _ = std::fs::write(&base_src, &identity);
    let original = verdict(args, test, &test.path, &dir.path().join("original"));
    let base = verdict(args, test, &base_src, &dir.path().join("identity"));
    if (original.status, &original.codes) != (base.status, &base.codes) {
        rec.skip = Some("printing changes the verdict".into());
        return rec;
    }
    let mut kept: Vec<(String, Vec<u8>)> = vec![("identity.rs".into(), identity.into_bytes())];
    let (mut found, mut notes) = (Vec::new(), Vec::new());
    for &name in names {
        if name == "identity"
            || (name == "reorder" && ORDER_MATTERS.is_match(&test.text))
            || NOISE.contains(&(test.rel.as_str(), name))
            // generic_const_exprs requires bounds in generic contexts that a concrete one does not.
            || (name == "generic-wrap" && test.text.contains("generic_const_exprs"))
        {
            continue;
        }
        let Outcome::Rewritten(text) = rewrite(name, &test.text) else { continue };
        rec.applied.push(name.into());
        let src = dir.path().join(format!("{name}.rs"));
        let _ = std::fs::write(&src, &text);
        let v = verdict(args, test, &src, &dir.path().join(name));
        let entry = |what: String| serde_json::json!({ "rewrite": name, "what": what, "codes": v.codes, "base_codes": base.codes, "stderr": v.stderr, "base_stderr": base.stderr });
        if v.status != base.status && v.status != Status::Timeout && base.status != Status::Timeout {
            found.push(entry(format!("verdict: {:?} -> {:?}", base.status, v.status).to_lowercase()));
            kept.push((format!("{name}.rs"), text.into_bytes()));
        } else if v.status == Status::Error && base.status == Status::Error && v.codes != base.codes {
            // Which error suppresses which may depend on order: a note.
            notes.push(entry(format!("codes: {:?} -> {:?}", base.codes, v.codes)));
        }
    }
    let label = |e: &serde_json::Value| format!("{}: {}", e["rewrite"].as_str().unwrap_or(""), e["what"].as_str().unwrap_or(""));
    rec.found = found.iter().map(label).collect();
    rec.notes = notes.iter().map(label).collect();
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &kept, &serde_json::json!({ "kind": test.kind, "found": found, "notes": notes }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let names: Vec<&str> = match &args.rewrites {
        Some(r) => r.split(',').collect(),
        None => REWRITES.to_vec(),
    };
    let tests = args.sweep.select(uitest::tests(&args.sweep.tests, uitest::ALL, |_| false));
    println!("{} tests, rewrites: {}", tests.len(), names.iter().filter(|n| **n != "identity").copied().collect::<Vec<_>>().join(", "));
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, &names, t)))
}

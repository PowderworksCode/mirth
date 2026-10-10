//! Suggestions apply: a machine-applicable suggestion must produce code that compiles the way
//! the suggestion promises.
//!
//! For each standalone UI test without `//@ run-rustfix` (compiletest checks those), applies
//! each `MachineApplicable` suggestion inside the test file alone and compiles again:
//!
//! - lint-breaks: a warning's (a lint's) fix introduces an error; lints never stop a build, and
//!   `cargo fix` applies their fixes without asking
//! - parse: the fixed file no longer parses
//! - not-fixed: not one fewer of the same diagnostic
//! - ice: the fixed file crashes the compiler
//!
//! Errors appearing after an error's suggestion is applied are expected and not reported.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{self, Compile, Diagnostic, Status};
use mirth_lab::uitest::{self, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// Include tests with `//@ run-rustfix`.
    #[arg(long)]
    with_rustfix: bool,
    /// Suggestions tried per test.
    #[arg(long, default_value_t = 8)]
    max: usize,
    #[command(flatten)]
    sweep: Sweep,
}

static BY_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*;|include(_str|_bytes)?!|#\[path").unwrap());
static RUSTFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^//@\s*run-rustfix").unwrap());

type Part = (usize, usize, String);

#[derive(Serialize)]
struct Rec {
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    tried: usize,
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

fn diagnose(args: &Args, test: &Test, source: &Path, out: &Path) -> (Status, Vec<Diagnostic>) {
    let c = Compile::new(&args.rustc, source, out, &test.flags, test.edition())
        .emit("metadata")
        .json()
        .timeout(120)
        .run();
    (c.status, rustc::diagnostics(&c.stderr))
}

/// The machine-applicable suggestions of a diagnostic (and its direct children) in `file`.
fn suggestions(diag: &Diagnostic, file: &str) -> Vec<Vec<Part>> {
    std::iter::once(diag)
        .chain(diag.children.iter())
        .filter_map(|node| {
            let mut parts: Vec<Part> = node
                .spans
                .iter()
                .filter(|s| {
                    s.suggestion_applicability.as_deref() == Some("MachineApplicable")
                        && Path::new(&s.file_name).file_name().and_then(|n| n.to_str()) == Some(file)
                })
                .filter_map(|s| s.suggested_replacement.clone().map(|r| (s.byte_start, s.byte_end, r)))
                .collect();
            parts.sort();
            (!parts.is_empty()).then_some(parts)
        })
        .collect()
}

fn key(d: &Diagnostic) -> (String, String) {
    (d.code().to_owned(), d.message.clone())
}

/// The errors, by code (or lint name) when they have one: a renamed identifier changes the
/// message of the same lint.
fn errors(diags: &[Diagnostic]) -> BTreeSet<(String, String)> {
    diags
        .iter()
        .filter(|d| d.level == "error" && !d.message.starts_with("aborting"))
        .map(|d| if d.code().is_empty() { (String::new(), d.message.clone()) } else { (d.code().to_owned(), String::new()) })
        .collect()
}

fn check(args: &Args, test: &Test) -> Rec {
    let mut rec = Rec { test: test.rel.clone(), skip: None, tried: 0, found: Vec::new() };
    if BY_PATH.is_match(&test.text) {
        rec.skip = Some("uses files by path".into());
        return rec;
    }
    let dir = driver::scratch_dir(&args.sweep);
    let src = dir.path().join(test.file_name());
    let _ = std::fs::copy(&test.path, &src);
    let (status, diags) = diagnose(args, test, &src, &dir.path().join("orig"));
    if matches!(status, Status::Ice | Status::Timeout) {
        rec.skip = Some(format!("original {status:?}").to_lowercase());
        return rec;
    }
    let base_errors = errors(&diags);
    let text = std::fs::read(&src).unwrap_or_default();
    let mut found: Vec<serde_json::Value> = Vec::new();
    let mut kept: Vec<(String, Vec<u8>)> = Vec::new();
    'outer: for diag in &diags {
        for parts in suggestions(diag, test.file_name()) {
            if rec.tried >= args.max {
                break 'outer;
            }
            rec.tried += 1;
            // Apply from the end so earlier offsets stay valid; overlapping parts are skipped.
            let mut fixed = text.clone();
            let mut last: Option<usize> = None;
            let mut ok = true;
            for (start, end, repl) in parts.iter().rev() {
                if last.is_some_and(|l| *end > l) || *end > fixed.len() || start > end {
                    ok = false;
                    break;
                }
                fixed.splice(*start..*end, repl.bytes());
                last = Some(*start);
            }
            if !ok {
                continue;
            }
            let fdir = dir.path().join(format!("fix{}", rec.tried));
            let _ = std::fs::create_dir_all(&fdir);
            let fsrc = fdir.join(test.file_name());
            let _ = std::fs::write(&fsrc, &fixed);
            let (fstatus, fdiags) = diagnose(args, test, &fsrc, &fdir);
            let new: BTreeSet<_> = errors(&fdiags).difference(&base_errors).cloned().collect();
            let what = if fstatus == Status::Ice {
                Some("ice")
            } else if new.iter().any(|(_, m)| m.contains("expected") || m.contains("unexpected") || m.contains("unknown start of token")) {
                Some("parse")
            } else if diag.level == "warning" && !new.is_empty() {
                Some("lint-breaks")
            } else if fdiags.iter().filter(|d| key(d) == key(diag)).count() >= diags.iter().filter(|d| key(d) == key(diag)).count() {
                // Nested braces legitimately report the next level, but then there is one fewer.
                Some("not-fixed")
            } else {
                None
            };
            if let Some(what) = what {
                let name = format!("fix{}.rs", rec.tried);
                found.push(serde_json::json!({
                    "what": what, "diagnostic": diag.message, "code": diag.code(), "level": diag.level,
                    "parts": parts, "fixed_name": name,
                    "new_errors": new.iter().map(|(c, m)| if c.is_empty() { m.clone() } else { c.clone() }).take(5).collect::<Vec<_>>(),
                }));
                kept.push((name, fixed));
            }
        }
    }
    rec.found = found
        .iter()
        .map(|f| {
            let who = if f["code"].as_str().unwrap_or("").is_empty() { f["level"].as_str().unwrap_or("") } else { f["code"].as_str().unwrap_or("") };
            format!("{}: {} {}", f["what"].as_str().unwrap_or(""), who, f["diagnostic"].as_str().unwrap_or("").chars().take(80).collect::<String>())
        })
        .collect();
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &kept, &serde_json::json!({ "found": found }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let tests = uitest::tests(&args.sweep.tests, uitest::ALL, |t| !args.with_rustfix && RUSTFIX.is_match(&t.text));
    let tests = args.sweep.select(tests);
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, t)))
}

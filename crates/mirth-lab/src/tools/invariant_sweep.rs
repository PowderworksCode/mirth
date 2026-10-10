//! In-compiler invariants over the UI tests: each standalone test compiled by a compiler with
//! docs/hunt/check-invariants.patch, with `RUSTC_CHECK_INVARIANTS` set.
//!
//! The patch prints `rustc-invariant: <property>: <details>` for each violation and carries
//! on. Some checks it turns on are rustc's own debug-build assertions (layout sanity, argument
//! lists against generics in debug builds), which panic instead: an ICE that happens only with
//! the variable set is a finding too (`env-only-ice`). Tests that build (build-pass, run-pass,
//! *-fail past type checking) are compiled to a binary, so codegen's checks run; the rest to
//! metadata.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{Compile, Status};
use mirth_lab::uitest::{self, Kind, Test};
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// A rustc built with docs/hunt/check-invariants.patch.
    #[arg(long)]
    rustc: PathBuf,
    #[command(flatten)]
    sweep: Sweep,
}

const PREFIX: &str = "rustc-invariant: ";

#[derive(Serialize)]
struct Rec {
    test: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
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

/// The first line of a panic message in rustc's stderr.
fn panic_line(stderr: &str) -> String {
    let mut lines = stderr.lines();
    while let Some(l) = lines.next() {
        if l.contains("panicked at") {
            let msg = lines.next().unwrap_or("");
            return format!("{} {}", l.trim(), msg.trim()).chars().take(300).collect();
        }
    }
    stderr.lines().find(|l| l.contains("internal compiler error")).unwrap_or("").chars().take(300).collect()
}

fn check(args: &Args, test: &Test) -> Rec {
    let dir = driver::scratch_dir(&args.sweep);
    let emit = if Kind::is_check(test.kind) { "metadata" } else { "link" };
    let compile = |env: bool| {
        let c = Compile::new(&args.rustc, &test.path, dir.path(), &test.flags, test.edition()).emit(emit).timeout(180);
        if env { c.env("RUSTC_CHECK_INVARIANTS", "1") } else { c }.run()
    };
    let c = compile(true);
    let mut rec = Rec { test: test.rel.clone(), status: format!("{:?}", c.status).to_lowercase(), skip: None, found: Vec::new() };
    let mut found: BTreeSet<String> = c.stderr.lines().filter_map(|l| l.strip_prefix(PREFIX)).map(str::to_owned).collect();
    match c.status {
        Status::Timeout => rec.skip = Some("timeout".into()),
        Status::Ice => {
            let without = compile(false);
            if without.status == Status::Ice {
                rec.skip = Some("ice without the checks too".into());
            } else {
                found.insert(format!("env-only-ice: {}", panic_line(&c.stderr)));
            }
        }
        _ => {}
    }
    rec.found = found.into_iter().collect();
    if !rec.found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &[], &serde_json::json!({ "found": rec.found, "emit": emit }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let tests = uitest::tests(&args.sweep.tests, uitest::ALL, |_| false);
    let tests = args.sweep.select(tests);
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, t)))
}

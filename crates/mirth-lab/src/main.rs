//! `mirth-lab <check> …`: the checks over rustc. Each subcommand's module documents what it
//! looks for; docs/checks.md has what each found.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod tools {
    pub mod crash_diff;
    pub mod diag_check;
    pub mod miri_diff;
    pub mod opt_diff;
    pub mod rewrite_diff;
    pub mod solver_diff;
    pub mod suggest_diff;
}

#[derive(Parser)]
#[command(name = "mirth-lab", about = "Checks (oracles) over rustc")]
struct Cli {
    #[command(subcommand)]
    check: Check,
}

#[derive(Subcommand)]
enum Check {
    /// Behavior must not depend on optimization (opt levels, MIR opt levels, LTO, Cranelift).
    OptDiff(tools::opt_diff::Args),
    /// The old and new trait solvers, and NLL and Polonius, must agree.
    SolverDiff(tools::solver_diff::Args),
    /// rustc's internal checks (debug assertions, MIR validation) on every UI test.
    CrashDiff(tools::crash_diff::Args),
    /// Invariants of every diagnostic (no internal debug output, spans in bounds).
    DiagCheck(tools::diag_check::Args),
    /// Accepted safe programs are UB-free under Miri; MIR optimizations and native code agree with Miri.
    MiriDiff(tools::miri_diff::Args),
    /// Meaning-preserving rewrites (generic-wrap, alias, reorder, unused) keep the verdict.
    RewriteDiff(tools::rewrite_diff::Args),
    /// Machine-applicable suggestions, applied one at a time, keep the program compiling.
    SuggestDiff(tools::suggest_diff::Args),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.check {
        Check::OptDiff(a) => tools::opt_diff::run(a),
        Check::SolverDiff(a) => tools::solver_diff::run(a),
        Check::CrashDiff(a) => tools::crash_diff::run(a),
        Check::DiagCheck(a) => tools::diag_check::run(a),
        Check::MiriDiff(a) => tools::miri_diff::run(a),
        Check::RewriteDiff(a) => tools::rewrite_diff::run(a),
        Check::SuggestDiff(a) => tools::suggest_diff::run(a),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("mirth-lab: {e:#}");
            ExitCode::from(2)
        }
    }
}

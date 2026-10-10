//! `mirth-lab <check> …`: the checks over rustc. Each subcommand's module documents what it
//! looks for; docs/checks.md has what each found.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod tools {
    pub mod abi_diff;
    pub mod audit_options;
    pub mod callgraph;
    pub mod crash_diff;
    pub mod diag_check;
    pub mod flag_fuzz;
    pub mod flag_min;
    pub mod flag_model;
    pub mod flag_rows;
    pub mod flag_universe;
    pub mod flag_walk;
    pub mod gate_check;
    pub mod instr_check;
    pub mod miri_diff;
    pub mod opt_diff;
    pub mod release_diff;
    pub mod repro_diff;
    pub mod rewrite_diff;
    pub mod scale_check;
    pub mod solver_diff;
    pub mod suggest_diff;
    pub mod ui_fuzz;
    pub mod xlink;
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
    /// Outputs depend only on inputs: repeat, other directory, threads, decoy libraries.
    ReproDiff(tools::repro_diff::Args),
    /// Nothing unstable is usable from stable code (attributes, library items).
    GateCheck(tools::gate_check::Args),
    /// PGO and coverage instrumentation round trips.
    InstrCheck(tools::instr_check::Args),
    /// Real crates accepted by one toolchain are accepted by the next, in comparable time.
    ReleaseDiff(tools::release_diff::Args),
    /// Every target builds core and alloc and links a program with no undefined symbols.
    Xlink(tools::xlink::Args),
    /// Compile time, memory, frames and future sizes grow about linearly with program size.
    ScaleCheck(tools::scale_check::Args),
    /// rustc's extern "C" lowering matches clang's for random C signatures, per target.
    AbiDiff(tools::abi_diff::Args),
    /// Incremental rebuilds of UI tests after random edits match clean builds.
    UiFuzz(tools::ui_fuzz::Args),
    /// Reachability over the compiler's call graph; coverage of what can run, and gap lists.
    Callgraph(tools::callgraph::Args),
    /// rustc's -C and -Z options: domains, values and pairs accepted, covering-array sizes.
    FlagUniverse(tools::flag_universe::Args),
    /// A PICT model of the option universe (optionally of transitions, for a Cargo build).
    FlagModel(tools::flag_model::Args),
    /// Compile a trivial crate per PICT row; count the rejected rows by first error.
    FlagRows(tools::flag_rows::Args),
    /// Option transitions between incremental sessions: the rebuild must match a clean build.
    FlagWalk(tools::flag_walk::Args),
    /// Delta-debug the options of a flag walk's failing rows to a minimal set per error.
    FlagMin(tools::flag_min::Args),
    /// The fuzzer under the option configurations of a PICT table.
    FlagFuzz(tools::flag_fuzz::Args),
    /// [UNTRACKED] options must not change what incremental compilation reuses.
    AuditOptions(tools::audit_options::Args),
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
        Check::ReproDiff(a) => tools::repro_diff::run(a),
        Check::GateCheck(a) => tools::gate_check::run(a),
        Check::InstrCheck(a) => tools::instr_check::run(a),
        Check::ReleaseDiff(a) => tools::release_diff::run(a),
        Check::Xlink(a) => tools::xlink::run(a),
        Check::ScaleCheck(a) => tools::scale_check::run(a),
        Check::AbiDiff(a) => tools::abi_diff::run(a),
        Check::UiFuzz(a) => tools::ui_fuzz::run(a),
        Check::Callgraph(a) => tools::callgraph::run(a),
        Check::FlagUniverse(a) => tools::flag_universe::run(a),
        Check::FlagModel(a) => tools::flag_model::run(a),
        Check::FlagRows(a) => tools::flag_rows::run(a),
        Check::FlagWalk(a) => tools::flag_walk::run(a),
        Check::FlagMin(a) => tools::flag_min::run(a),
        Check::FlagFuzz(a) => tools::flag_fuzz::run(a),
        Check::AuditOptions(a) => tools::audit_options::run(a),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("mirth-lab: {e:#}");
            ExitCode::from(2)
        }
    }
}

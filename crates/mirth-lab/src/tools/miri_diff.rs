//! Miri differential: an accepted safe program must be free of undefined behavior, MIR
//! optimizations must not introduce any, and the compiled program must do what Miri says.
//!
//! For each runnable UI test, interprets it with Miri at `-Zmir-opt-level` 0, 2 and 4 and builds
//! and runs it natively. Findings: UB at level 0 in a test without `unsafe` code; UB only after
//! MIR optimization; Miri's exit status or stdout changing with the MIR level; the native
//! program's exit status or stdout differing from Miri's. Tests Miri cannot run are skipped, as
//! are comparisons for threaded tests (scheduling differs) and tests asserting what Rust leaves
//! unspecified (function pointer equality, zero-sized addresses), which Miri varies on purpose.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::miri::{Miri, MiriRun, MiriStatus};
use mirth_lab::normalize;
use mirth_lab::rustc::{self, Compile, Exit};
use mirth_lab::uitest::{self, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    #[arg(long, default_value = "nightly-2026-10-06")]
    miri_toolchain: String,
    /// Seconds per Miri run.
    #[arg(long, default_value_t = 120)]
    timeout: u64,
    #[command(flatten)]
    sweep: Sweep,
}

const FIXED: &[&str] = &["-Coverflow-checks=on", "-Cdebug-assertions=on"];
/// Miri without preemption: threads switch only where they block, the same at every MIR level.
const MIRI_FLAGS: &[&str] = &["-Zmiri-preemption-rate=0"];
const LEVELS: &[(&str, &[&str])] = &[("miri0", &[]), ("miri2", &["-Zmir-opt-level=2"]), ("miri4", &["-Zmir-opt-level=4"])];
/// Tests asserting what Rust leaves unspecified, which Miri varies on purpose: function pointer
/// equality, the addresses of zero-sized values, stack addresses, function alignment; and one
/// Miri limitation (`.init_array` functions called without glibc's arguments).
const UNSPECIFIED: &[&str] = &[
    "consts/const-extern-function.rs",
    "consts/zst_no_llvm_alloc.rs",
    "layout/null-pointer-optimization.rs",
    "mir/mir_misc_casts.rs",
    "mir/mir_coercions.rs",
    "extern/extern-compare-with-return-type.rs",
    "fn/fn-ptr-trait-run.rs",
    "mir/mir_raw_fat_ptr.rs",
    "codegen/StackColoring-not-blowup-stack-issue-40883.rs",
    "attributes/fn-align-dyn.rs",
    "runtime/stdout-before-main.rs",
];
static THREADS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"thread::(spawn|scope)|std::sync::mpsc|\bspawn\(").unwrap());
static OWN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^-O$|opt-level|overflow-checks|debug-assertions|codegen-backend|mir-enable-passes|panic=|-Cpanic|prefer-dynamic|-Zbuild-std|-Clink|-Ctarget").unwrap()
});
/// Code Miri cannot interpret, and compile-time output that would land in Miri's stdout.
static NOT_FOR_MIRI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\basm!|global_asm!|naked_asm!|extern\s+"C"\s*\{|#\[link\(|std::process::Command|\bfork\b|libc::|dlopen|std::os::unix::process|trace_macros|log_syntax"#).unwrap()
});

#[derive(Serialize)]
struct Rec {
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    miri: Vec<(String, MiriStatus)>,
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

fn clean(text: &str, test: &Test) -> String {
    // argv[0]: the source file under Miri, the binary natively.
    let t = text.replace(&test.path.to_string_lossy().into_owned(), "<argv0>");
    let t = regex_argv0().replace_all(&t, "<argv0>").into_owned();
    normalize::stdout(&normalize::stderr(&t))
}

fn regex_argv0() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\S*/native/prog\b").unwrap());
    &RE
}

fn tail(s: &str, n: usize) -> String {
    let v: Vec<char> = s.chars().collect();
    v[v.len().saturating_sub(n)..].iter().collect()
}

fn check(args: &Args, miri: &Miri, test: &Test) -> Rec {
    let dir = driver::scratch_dir(&args.sweep);
    let mut rec = Rec { test: test.rel.clone(), skip: None, note: None, miri: Vec::new(), found: Vec::new() };
    let native = Compile::new(&args.rustc, &test.path, &dir.path().join("native"), &test.flags, test.edition())
        .extra(FIXED.iter().copied().chain(["-Copt-level=0"]))
        .run();
    let Some(binary) = native.binary else {
        rec.skip = Some(format!("native build {:?}", native.status).to_lowercase());
        return rec;
    };
    let mut outs: Vec<(Exit, String)> =
        (0..3).map(|_| rustc::observe(&binary, 20, &[])).map(|o| (o.exit, clean(&o.stdout, test))).collect();
    outs.dedup();
    if outs.len() > 1 {
        rec.skip = Some("native run is nondeterministic".into());
        return rec;
    }
    let (native_exit, native_out) = outs.pop().unwrap();
    let mut runs: Vec<(&str, MiriRun)> = Vec::new();
    for &(name, extra) in LEVELS {
        let flags: Vec<String> = FIXED.iter().chain(MIRI_FLAGS).chain(extra).map(|s| s.to_string()).collect();
        let mut m = miri.run(&test.path, &test.flags, test.edition(), &flags, args.timeout, dir.path());
        m.stdout = clean(&m.stdout, test);
        if name == "miri0" && matches!(m.status, MiriStatus::Unsupported | MiriStatus::Error | MiriStatus::Timeout) {
            rec.skip = Some(format!("miri {:?}", m.status).to_lowercase());
            return rec;
        }
        runs.push((name, m));
    }
    rec.miri = runs.iter().map(|(n, m)| (n.to_string(), m.status)).collect();
    let unspecified = UNSPECIFIED.contains(&test.rel.as_str());
    let threaded = THREADS.is_match(&test.text);
    let safe = !test.text.contains("unsafe");
    let m0 = &runs[0].1;
    let mut found: Vec<serde_json::Value> = Vec::new();
    // UB in a program without `unsafe` code can only be the compiler's.
    if m0.status == MiriStatus::Ub && safe && !unspecified {
        found.push(serde_json::json!({ "what": "ub (safe code)", "stderr": tail(&m0.stderr, 3000) }));
    } else if m0.status == MiriStatus::Ub {
        rec.note = Some("ub in a test with unsafe code".into());
    }
    for (name, m) in &runs[1..] {
        if m.status == MiriStatus::Ub && m0.status != MiriStatus::Ub {
            found.push(serde_json::json!({ "what": format!("ub-opt ({name})"), "stderr": tail(&m.stderr, 3000) }));
        } else if m.status == MiriStatus::Ice {
            found.push(serde_json::json!({ "what": format!("ice ({name})"), "stderr": tail(&m.stderr, 3000) }));
        } else if m.status == MiriStatus::Ok && m0.status == MiriStatus::Ok && !threaded && (&m.exit, &m.stdout) != (&m0.exit, &m0.stdout) {
            found.push(serde_json::json!({ "what": format!("opt-differs ({name})"), "miri0": tail(&m0.stdout, 1500), "got": tail(&m.stdout, 1500) }));
        }
    }
    if m0.status == MiriStatus::Ok && !threaded && !unspecified {
        // Miri exits 1 on a panic that reaches main, native code 101.
        let exit_m = if m0.exit == Exit::Code(1) && m0.stderr.contains("panicked") { Exit::Code(101) } else { m0.exit.clone() };
        if (exit_m, &m0.stdout) != (native_exit.clone(), &native_out) {
            found.push(serde_json::json!({ "what": "native", "miri": [m0.exit, tail(&m0.stdout, 1500)], "native": [native_exit, tail(&native_out, 1500)] }));
        }
    }
    rec.found = found.iter().map(|f| f["what"].as_str().unwrap_or("").to_owned()).collect();
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &[], &serde_json::json!({ "fixed": FIXED, "found": found }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let miri = Miri::pinned(&args.miri_toolchain);
    let tests = uitest::tests(&args.sweep.tests, uitest::RUNNABLE, |t| uitest::flag_matches(t, &OWN) || NOT_FOR_MIRI.is_match(&t.text));
    let tests = args.sweep.select(tests);
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, &miri, t)))
}

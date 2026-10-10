//! Optimization differential: a program's behavior must not depend on how it was optimized.
//!
//! Builds each runnable UI test under a set of configurations (optimization levels, MIR
//! optimization levels, LTO, target CPU, the Cranelift backend) and runs it, with overflow checks
//! and debug assertions fixed, and compares exit status, stdout and stderr with the unoptimized
//! baseline (`-Copt-level=0 -Zmir-opt-level=0`). A baseline whose output varies between two runs
//! is skipped; a difference counts only if the configuration's binary repeats it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::normalize;
use mirth_lab::rustc::{self, Compile, Observed, Status};
use mirth_lab::uitest::{self, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// A rustc with the Cranelift backend (the pinned nightly's).
    #[arg(long)]
    cranelift: Option<PathBuf>,
    /// Comma-separated subset of configurations (`base` is always built).
    #[arg(long)]
    configs: Option<String>,
    #[command(flatten)]
    sweep: Sweep,
}

const FIXED: &[&str] = &["-Coverflow-checks=on", "-Cdebug-assertions=on", "-Cpanic=unwind", "-Cdebuginfo=0"];
const CONFIGS: &[(&str, &[&str])] = &[
    ("base", &["-Copt-level=0", "-Zmir-opt-level=0"]),
    ("O0", &["-Copt-level=0"]),
    ("O0-mir4", &["-Copt-level=0", "-Zmir-opt-level=4"]),
    ("O1", &["-Copt-level=1"]),
    ("O2", &["-Copt-level=2"]),
    ("O3", &["-Copt-level=3"]),
    ("Os", &["-Copt-level=s"]),
    ("Oz", &["-Copt-level=z"]),
    ("O3-mir4", &["-Copt-level=3", "-Zmir-opt-level=4"]),
    ("O3-lto", &["-Copt-level=3", "-Clto=fat", "-Ccodegen-units=1"]),
    ("O2-cgu16", &["-Copt-level=2", "-Ccodegen-units=16"]),
    ("O3-native", &["-Copt-level=3", "-Ctarget-cpu=native"]),
    ("cranelift", &["-Copt-level=0", "-Zcodegen-backend=cranelift"]),
];
/// Tests that choose these themselves are left out: the configuration would contradict them.
static OWN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^-O$|opt-level|mir-opt-level|overflow-checks|debug-assertions|codegen-backend|mir-enable-passes|^-Clto|lto=|target-cpu|panic=|-Cpanic|prefer-dynamic|-Zbuild-std").unwrap()
});
/// Tests whose outcome legitimately depends on optimization: unspecified behavior (whether two
/// equal promoted constants share an address), stack usage, a backend's documented gaps.
const NOISE: &[(&str, &[&str])] = &[
    ("mir/mir_raw_fat_ptr.rs", &["cranelift"]),
    ("codegen/StackColoring-not-blowup-stack-issue-40883.rs", &["O0-mir4", "O0"]),
    ("attributes/fn-align-dyn.rs", &["cranelift"]),
    ("backtrace/backtrace.rs", &["cranelift"]),
];

#[derive(Serialize)]
struct Finding {
    config: String,
    what: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<Observed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    got: Option<Observed>,
    #[serde(skip_serializing_if = "String::is_empty")]
    stderr: String,
}

#[derive(Serialize)]
struct Rec {
    test: String,
    build: BTreeMap<String, Status>,
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

fn observe(binary: &Path) -> Observed {
    // Every configuration's program runs from the same path: some tests print argv[0].
    let fixed = binary.parent().and_then(Path::parent).unwrap_or(Path::new(".")).join("run").join("prog");
    let _ = std::fs::create_dir_all(fixed.parent().unwrap());
    let _ = std::fs::copy(binary, &fixed);
    let o = rustc::observe(&fixed, 20, &[]);
    Observed { exit: o.exit, stdout: normalize::stdout(&o.stdout), stderr: normalize::stderr(&o.stderr) }
}

fn check(args: &Args, configs: &[(&str, &[&str])], test: &Test) -> Rec {
    let dir = driver::scratch_dir(&args.sweep);
    let mut rec = Rec { test: test.rel.clone(), build: BTreeMap::new(), skip: None, found: Vec::new() };
    let mut runs: BTreeMap<&str, Observed> = BTreeMap::new();
    let mut stderrs: BTreeMap<&str, String> = BTreeMap::new();
    let unwinding = test.text.contains("catch_unwind") || test.text.contains("needs-unwind");
    for &(name, cfg) in configs {
        // Cranelift does not unwind on this target yet (catch_unwind catches nothing).
        if name == "cranelift" && unwinding {
            continue;
        }
        let rustc = if name == "cranelift" { args.cranelift.as_deref().unwrap() } else { args.rustc.as_path() };
        let out = dir.path().join(name);
        let c = Compile::new(rustc, &test.path, &out, &test.flags, test.edition())
            .extra(FIXED.iter().chain(cfg.iter()).copied())
            .run();
        rec.build.insert(name.to_owned(), c.status);
        stderrs.insert(name, c.stderr.chars().rev().take(2000).collect::<String>().chars().rev().collect());
        if let Some(b) = c.binary {
            runs.insert(name, observe(&b));
        }
    }
    if rec.build.get("base") != Some(&Status::Ok) || !runs.contains_key("base") {
        rec.skip = Some("baseline does not build".into());
        return rec;
    }
    let base = runs["base"].clone();
    let again = observe(&dir.path().join("base").join("prog"));
    if again != base {
        rec.skip = Some(format!("baseline is nondeterministic: {:?} vs {:?}", base, again).chars().take(600).collect());
        return rec;
    }
    let noise: &[&str] = NOISE.iter().find(|(t, _)| *t == test.rel).map_or(&[], |(_, c)| c);
    let mut found = Vec::new();
    for &(name, _) in configs {
        if name == "base" || !rec.build.contains_key(name) || noise.contains(&name) {
            continue;
        }
        let status = rec.build[name];
        if status != Status::Ok {
            // Cranelift's documented gaps (tail calls, some linkages and SIMD intrinsics) show as
            // errors or as panics inside the backend.
            if name == "cranelift" && (status == Status::Error || stderrs[name].contains("rustc_codegen_cranelift")) {
                continue;
            }
            found.push(Finding { config: name.into(), what: format!("build {status:?}").to_lowercase(), base: None, got: None, stderr: stderrs[name].clone() });
            continue;
        }
        let Some(got) = runs.get(name) else { continue };
        if name == "cranelift" && got.stderr.contains("failed to initiate panic") {
            continue;
        }
        if *got != base {
            let retry = observe(&dir.path().join(name).join("prog"));
            if retry != base && retry == *got {
                let diff: Vec<&str> = [("exit", got.exit != base.exit), ("stdout", got.stdout != base.stdout), ("stderr", got.stderr != base.stderr)]
                    .into_iter()
                    .filter_map(|(k, d)| d.then_some(k))
                    .collect();
                found.push(Finding { config: name.into(), what: format!("run differs: {}", diff.join(",")), base: Some(base.clone()), got: Some(got.clone()), stderr: String::new() });
            }
        }
    }
    rec.found = found.iter().map(|f| format!("{}: {}", f.config, f.what)).collect();
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &[], &serde_json::json!({ "fixed": FIXED, "found": found }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let wanted: Option<Vec<&str>> = args.configs.as_deref().map(|c| c.split(',').collect());
    let configs: Vec<(&str, &[&str])> = CONFIGS
        .iter()
        .copied()
        .filter(|(n, _)| *n == "base" || wanted.as_ref().is_none_or(|w| w.contains(n)))
        .filter(|(n, _)| *n != "cranelift" || args.cranelift.is_some())
        .collect();
    let tests = uitest::tests(&args.sweep.tests, uitest::RUNNABLE, |t| uitest::flag_matches(t, &OWN));
    let tests = args.sweep.select(tests);
    println!("{} tests, configurations: {}", tests.len(), configs.iter().map(|c| c.0).collect::<Vec<_>>().join(", "));
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, &configs, t)))
}

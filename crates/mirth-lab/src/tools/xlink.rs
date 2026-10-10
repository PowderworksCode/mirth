//! Cross-target build and link: every target must build `core` and `alloc` and link a program
//! with its documented linker, with no undefined symbols.
//!
//! For each target, builds rustc/xlink-probe (a `no_std` program needing compiler-builtins: 128-bit
//! integers, float conversions and math, float formatting, large copies, atomics) with
//! `cargo -Zbuild-std=core,alloc` and links it; targets whose default linker is a C compiler
//! driver link with `rust-lld` in the flavor their spec names. Results: ok, link-undefined (the
//! finding: lld reports undefined symbols), link, env (a library or startup file of the target's
//! C sysroot missing here), build, ice, skipped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;
use std::time::Duration;

use mirth_lab::rustc::{Exit, run_command};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    toolchain: String,
    #[arg(long)]
    work: PathBuf,
    /// Comma-separated targets (default: every target rustc knows).
    #[arg(long)]
    targets: Option<String>,
    #[arg(long, default_value_t = 6)]
    jobs: usize,
    /// The probe crate (default: rustc/xlink-probe in the mirth checkout).
    #[arg(long)]
    probe: Option<PathBuf>,
}

/// Targets that need more than a target name (a CPU, a linker that is not lld).
static SKIP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(amdgcn|nvptx|bpf|spirv)|avr-none").unwrap());
static UNDEFINED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"undefined symbol: (\S+)").unwrap());
static ENV_MISSING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"unable to find library|cannot open crt|cannot open .*\.o\b|No such file").unwrap());

/// The parts of a target spec the link options depend on.
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct Spec {
    #[serde(default)]
    linker_flavor: String,
    #[serde(default)]
    linker: String,
    #[serde(default)]
    is_like_wasm: bool,
    #[serde(default)]
    is_like_msvc: bool,
    #[serde(default)]
    is_like_darwin: bool,
}

#[derive(Serialize)]
struct Res {
    result: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    flags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    undefined: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    first: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    tail: String,
}

fn link_flags(spec: &Spec) -> Vec<String> {
    let f = spec.linker_flavor.as_str();
    let own_lld = spec.linker.contains("lld") || f.ends_with("-lld") || f.starts_with("wasm-lld");
    let mut flags: Vec<&str> = Vec::new();
    if spec.is_like_wasm || f.starts_with("wasm") {
        flags.extend(["-Clink-arg=--no-entry", "-Clink-arg=--export=probe_entry"]);
    } else if spec.is_like_msvc || f.starts_with("msvc") {
        if !own_lld {
            flags.extend(["-Clinker=rust-lld", "-Clinker-flavor=lld-link"]);
        }
        flags.extend(["-Clink-arg=/ENTRY:probe_entry", "-Clink-arg=/NODEFAULTLIB"]);
    } else if spec.is_like_darwin || f.starts_with("darwin") {
        if !own_lld {
            flags.extend(["-Clinker=rust-lld", "-Clinker-flavor=ld64.lld"]);
        }
        flags.extend(["-Clink-arg=-e", "-Clink-arg=_probe_entry", "-Clink-arg=-undefined", "-Clink-arg=dynamic_lookup"]);
    } else {
        if !own_lld {
            flags.extend(["-Clinker=rust-lld", "-Clinker-flavor=ld.lld"]);
        }
        flags.push("-Clink-arg=--entry=probe_entry");
    }
    flags.into_iter().map(str::to_owned).collect()
}

fn rustc_out(toolchain: &str, a: &[&str]) -> Option<String> {
    let out = Command::new("rustc").arg(format!("+{toolchain}")).args(a).env("RUSTC_BOOTSTRAP", "1").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn one(args: &Args, probe: &Path, target: &str) -> Res {
    let skipped = |why: &str| Res { result: "skipped".into(), flags: vec![], undefined: vec![], first: why.into(), tail: String::new() };
    if SKIP.is_match(target) {
        return skipped("");
    }
    let Some(spec_json) = rustc_out(&args.toolchain, &["--print", "target-spec-json", "-Zunstable-options", "--target", target]) else {
        return skipped("no spec");
    };
    let spec: Spec = serde_json::from_str(&spec_json).unwrap_or_default();
    let flags = link_flags(&spec);
    let tdir = args.work.join("target").join(target);
    let mut cmd = Command::new("cargo");
    cmd.arg(format!("+{}", args.toolchain))
        .args(["build", "--release", "-Zbuild-std=core,alloc", "-Zbuild-std-features=compiler-builtins-mem", "--target", target])
        .current_dir(probe)
        .env("CARGO_TARGET_DIR", &tdir)
        .env("RUSTFLAGS", flags.join(" "))
        .env("CARGO_TERM_COLOR", "never")
        .env_remove("RUSTC_WRAPPER");
    let done = run_command(cmd, Duration::from_secs(1500));
    let _ = std::fs::remove_dir_all(&tdir);
    let (exit, err) = match done {
        Ok(d) => (d.exit.clone(), d.stderr_text()),
        Err(e) => (Exit::Code(-1), e.to_string()),
    };
    let result = if exit == Exit::Code(0) {
        "ok"
    } else if exit == Exit::Timeout {
        "timeout"
    } else if err.contains("internal compiler error") || err.contains("panicked at") {
        "ice"
    } else if err.contains("linking with") || err.contains("lld: error") {
        if err.contains("undefined symbol") || err.contains("undefined reference") {
            "link-undefined"
        } else if ENV_MISSING.is_match(&err) {
            "env"
        } else {
            "link"
        }
    } else {
        "build"
    };
    let mut undefined: Vec<String> = UNDEFINED.captures_iter(&err).map(|c| c[1].to_owned()).collect();
    undefined.sort();
    undefined.dedup();
    undefined.truncate(20);
    let first = err.lines().find(|l| l.contains("error[") || l.contains("error:")).unwrap_or("").chars().take(300).collect();
    let tail = if result == "ok" { String::new() } else { err.chars().rev().take(2500).collect::<String>().chars().rev().collect() };
    Res { result: result.into(), flags, undefined, first, tail }
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let probe = args.probe.clone().unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../rustc/xlink-probe"));
    let targets: Vec<String> = match &args.targets {
        Some(t) => t.split(',').map(str::to_owned).collect(),
        None => rustc_out(&args.toolchain, &["--print", "target-list"]).unwrap_or_default().split_whitespace().map(str::to_owned).collect(),
    };
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;
    let results: BTreeMap<String, Res> = pool.install(|| targets.par_iter().map(|t| (t.clone(), one(&args, &probe, t))).collect());
    std::fs::write(args.work.join("results.json"), serde_json::to_string_pretty(&results)?)?;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for r in results.values() {
        *counts.entry(r.result.as_str()).or_default() += 1;
    }
    println!("{counts:?}");
    for (t, r) in &results {
        if r.result != "ok" && r.result != "skipped" && r.result != "env" {
            let what = if r.undefined.is_empty() { r.first.chars().take(120).collect() } else { r.undefined.join(" ") };
            println!("{:15} {t:40} {what}", r.result);
        }
    }
    Ok(ExitCode::SUCCESS)
}

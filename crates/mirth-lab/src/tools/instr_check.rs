//! Instrumentation round trip: instrumented programs must behave as uninstrumented ones and
//! write profiles that LLVM's tools accept.
//!
//! For each runnable UI test, with a toolchain that ships the profiler runtime and llvm-tools:
//! PGO (`-Cprofile-generate`, run, `llvm-profdata merge`, `-Cprofile-use`, run) and coverage
//! (`-Cinstrument-coverage`, run, merge, `llvm-cov export`). Findings: behavior differing from
//! the plain build, no profile written, the LLVM tools failing or warning about corrupt data,
//! the compiler failing or crashing on `-Cprofile-use`. Threaded tests are left out (scheduling);
//! a test relying on the linker discarding an unused symbol (`linking/executable-no-mangle-strip`)
//! fails to link instrumented, as expected.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;
use std::time::Duration;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::normalize;
use mirth_lab::rustc::{self, Compile, Exit, Status, run_command};
use mirth_lab::uitest::{self, Kind, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// A toolchain with the profiler runtime and llvm-tools.
    #[arg(long)]
    toolchain: String,
    #[command(flatten)]
    sweep: Sweep,
}

static OWN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"profile|instrument-coverage|coverage-options|^-O$|opt-level|panic=|prefer-dynamic|codegen-backend|-Clto|lto=|no-prepopulate").unwrap()
});
static THREADS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"thread::(spawn|scope)|std::sync::mpsc|\bspawn\(").unwrap());
static BAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)corrupt|malformed|invalid|truncated|failed to|error").unwrap());

#[derive(Serialize)]
struct Rec {
    test: String,
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

struct Toolchain {
    rustc: PathBuf,
    tools: PathBuf,
}

fn tool(tc: &Toolchain, name: &str, args: &[String]) -> (Exit, String) {
    let mut cmd = Command::new(tc.tools.join(name));
    cmd.args(args);
    match run_command(cmd, Duration::from_secs(120)) {
        Ok(d) => {
            let out = d.stdout_text();
            let text = d.stderr_text() + &out[out.len().saturating_sub(200)..];
            (d.exit, text)
        }
        Err(e) => (Exit::Code(-1), e.to_string()),
    }
}

fn observed(binary: &Path, env: &[(&str, &str)]) -> (Exit, String) {
    // Every build runs from the same path (some tests print argv[0]); the test harness's
    // timings and result order are normalized.
    let fixed = binary.parent().and_then(Path::parent).unwrap_or(Path::new(".")).join("run").join("prog");
    let _ = std::fs::create_dir_all(fixed.parent().unwrap());
    let _ = std::fs::copy(binary, &fixed);
    let o = rustc::observe(&fixed, 30, env);
    (o.exit, normalize::stdout(&o.stdout))
}

fn profraws(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "profraw")).map(|e| e.path().to_string_lossy().into_owned()).collect())
        .unwrap_or_default()
}

fn tail(s: &str, n: usize) -> String {
    let v: Vec<char> = s.chars().collect();
    v[v.len().saturating_sub(n)..].iter().collect()
}

fn check(args: &Args, tc: &Toolchain, test: &Test) -> Rec {
    let mut rec = Rec { test: test.rel.clone(), skip: None, found: Vec::new() };
    let dir = driver::scratch_dir(&args.sweep);
    let d = dir.path();
    let compile = |name: &str, extra: Vec<String>| Compile::new(&tc.rustc, &test.path, &d.join(name), &test.flags, test.edition()).extra(extra).run();
    let plain = compile("plain", vec!["-Copt-level=1".into()]);
    let Some(plain_bin) = plain.binary else {
        rec.skip = Some(format!("plain build {:?}", plain.status).to_lowercase());
        return rec;
    };
    let base = observed(&plain_bin, &[]);
    if (0..2).any(|_| observed(&plain_bin, &[]) != base) {
        rec.skip = Some("nondeterministic".into());
        return rec;
    }
    let mut found: Vec<serde_json::Value> = Vec::new();
    // PGO: generate, merge, use.
    let raw = d.join("pgo-raw");
    let generated = compile("gen", vec!["-Copt-level=1".into(), format!("-Cprofile-generate={}", raw.display())]);
    match generated.binary {
        None => found.push(serde_json::json!({ "what": format!("profile-generate build {:?}", generated.status).to_lowercase(), "stderr": tail(&generated.stderr, 1500) })),
        Some(bin) => {
            let got = observed(&bin, &[]);
            if got != base {
                found.push(serde_json::json!({ "what": "profile-generate changes behavior", "base": base, "got": got }));
            }
            let raws = profraws(&raw);
            if raws.is_empty() && got.0 == Exit::Code(0) {
                found.push(serde_json::json!({ "what": "no raw profile written" }));
            } else if !raws.is_empty() {
                let merged = d.join("merged.profdata");
                let mut a = vec!["merge".to_string(), "-o".into(), merged.to_string_lossy().into_owned()];
                a.extend(raws);
                let (exit, out) = tool(tc, "llvm-profdata", &a);
                if exit != Exit::Code(0) || BAD.is_match(&out) {
                    found.push(serde_json::json!({ "what": "llvm-profdata merge", "exit": exit, "out": tail(&out, 1500) }));
                } else {
                    let used = compile("use", vec!["-Copt-level=2".into(), format!("-Cprofile-use={}", merged.display())]);
                    match (used.status, used.binary) {
                        (Status::Ice, _) => found.push(serde_json::json!({ "what": "profile-use ICE", "stderr": tail(&used.stderr, 2000) })),
                        (s, None) => found.push(serde_json::json!({ "what": format!("profile-use build {s:?}").to_lowercase(), "stderr": tail(&used.stderr, 1500) })),
                        (_, Some(ub)) => {
                            let got = observed(&ub, &[]);
                            if got != base {
                                found.push(serde_json::json!({ "what": "profile-use changes behavior", "base": base, "got": got }));
                            }
                        }
                    }
                }
            }
        }
    }
    // Coverage: instrument, merge, export.
    let cov = compile("cov", vec!["-Cinstrument-coverage".into()]);
    match cov.binary {
        None => found.push(serde_json::json!({ "what": format!("instrument-coverage build {:?}", cov.status).to_lowercase(), "stderr": tail(&cov.stderr, 1500) })),
        Some(bin) => {
            let craw = d.join("cov-raw");
            let _ = std::fs::create_dir_all(&craw);
            let pattern = craw.join("c-%p.profraw").to_string_lossy().into_owned();
            let got = observed(&bin, &[("LLVM_PROFILE_FILE", &pattern)]);
            if got != base {
                found.push(serde_json::json!({ "what": "instrument-coverage changes behavior", "base": base, "got": got }));
            }
            let raws = profraws(&craw);
            if !raws.is_empty() {
                let merged = d.join("cov.profdata");
                let mut a = vec!["merge".to_string(), "-sparse".into(), "-o".into(), merged.to_string_lossy().into_owned()];
                a.extend(raws);
                let (exit, out) = tool(tc, "llvm-profdata", &a);
                if exit != Exit::Code(0) || BAD.is_match(&out) {
                    found.push(serde_json::json!({ "what": "llvm-profdata merge (coverage)", "exit": exit, "out": tail(&out, 1500) }));
                } else {
                    let a = vec!["export".to_string(), "-summary-only".into(), format!("-instr-profile={}", merged.display()), bin.to_string_lossy().into_owned()];
                    let (exit, out) = tool(tc, "llvm-cov", &a);
                    // A binary with nothing to instrument (a test harness without tests) has an
                    // empty coverage map, which llvm-cov refuses.
                    if exit != Exit::Code(0) && !out.contains("no coverage data found") {
                        found.push(serde_json::json!({ "what": "llvm-cov export", "exit": exit, "out": tail(&out, 1500) }));
                    }
                }
            } else if got.0 == Exit::Code(0) {
                found.push(serde_json::json!({ "what": "no coverage profile written" }));
            }
        }
    }
    rec.found = found.iter().map(|f| f["what"].as_str().unwrap_or("").to_owned()).collect();
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &[], &serde_json::json!({ "found": found }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let home = PathBuf::from(std::env::var("HOME")?);
    let root = home.join(format!(".rustup/toolchains/{}-x86_64-unknown-linux-gnu", args.toolchain));
    let tc = Toolchain { rustc: root.join("bin/rustc"), tools: root.join("lib/rustlib/x86_64-unknown-linux-gnu/bin") };
    // Threaded tests' output order depends on scheduling, which instrumentation changes.
    // Output that depends on a random hash seed.
    const NOISE: &[&str] = &["collections/hashmap/hashmap-debug-format.rs"];
    let tests = uitest::tests(&args.sweep.tests, &[Some(Kind::RunPass)], |t| {
        uitest::flag_matches(t, &OWN) || THREADS.is_match(&t.text) || NOISE.contains(&t.rel.as_str())
    });
    let tests = args.sweep.select(tests);
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, &tc, t)))
}

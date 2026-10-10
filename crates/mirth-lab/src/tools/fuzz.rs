//! Make random edits to a fixture and check each incremental rebuild.
//!
//! Each worker keeps one copy of the fixture and repeats:
//!
//!   1. apply a random mechanical edit (`mutations`: a comment, a moved item, a changed
//!      literal, a new function, ...);
//!   2. rebuild incrementally; if the edit does not compile, revert it (the next build then also
//!      exercises recovery from a failed session);
//!   3. build the same source from scratch, at the same path;
//!   4. compare:
//!        P6     every .rmeta Cargo reports for a workspace member
//!        rlib   every rlib's members, object code included
//!        exe    the binary's bytes
//!        diag   the diagnostics each crate printed
//!        run    the binaries' output and exit status
//!        reuse  the compiler's own check of what it reused (RUSTC_VERIFY_REUSE,
//!               docs/hunt/verify-reuse.patch) found nothing stale
//!        P5     when anything differs, clean builds are made again; if two clean builds
//!               differ, that is reported instead (nondeterminism)
//!        ICE    neither build crashed the compiler
//!        split  both builds succeed or both fail
//!
//! Every --reset kept edits the worker starts again from the pristine fixture. A finding keeps
//! the edits since the last reset (history.json, which `fuzz-replay` replays), and both builds'
//! differing files and logs.
//!
//! Stop it early by creating <work>/STOP. Progress is in <work>/stats-w<k>.json. With
//! --pause-on-finding all workers stop at the first finding (<work>/PAUSED says which); patch
//! rustc and run again with the patched compiler.
//!
//! Seeds name the same edit sequence only within this implementation: the Python fuzzer drew
//! from Python's generator.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use regex::Regex;
use serde_json::{Value, json};

use mirth_lab::artifacts::{self, Collected};
use mirth_lab::cargo::{self, copy_tree, messages, relative, tail};
use mirth_lab::mutations;

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    #[arg(long)]
    rustc: String,
    #[arg(long)]
    fixture: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value_t = 8)]
    workers: u64,
    /// Per worker.
    #[arg(long, default_value_t = 1_000_000_000)]
    edits: u64,
    #[arg(long, default_value_t = 40)]
    reset: usize,
    /// Findings kept per kind.
    #[arg(long, default_value_t = 5)]
    keep: usize,
    #[arg(long, default_value = "nightly-2026-10-06")]
    toolchain: String,
    #[arg(long, default_value = "-Zincremental-verify-ich", allow_hyphen_values = true)]
    rustflags: String,
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Seconds before a build counts as hung.
    #[arg(long, default_value_t = 180)]
    timeout: u64,
    /// Clean builds made again when anything differs, to tell nondeterminism from P6.
    #[arg(long, default_value_t = 12)]
    p5_builds: usize,
    /// Stop when the disk has less free.
    #[arg(long, default_value_t = 20)]
    min_free_gb: u64,
    /// cargo check instead of cargo build: metadata only, no code or binaries.
    #[arg(long)]
    check: bool,
    /// Do not set RUSTC_VERIFY_REUSE (needs a compiler with docs/hunt/verify-reuse.patch).
    #[arg(long)]
    no_verify_reuse: bool,
    /// Pass --target to cargo, so RUSTFLAGS skip build scripts and proc macros.
    #[arg(long)]
    target: Option<String>,
    /// Stop all workers at the first finding (writes <work>/PAUSED); patch rustc and run again.
    #[arg(long)]
    pause_on_finding: bool,
}

struct Ctx {
    args: Args,
    work: PathBuf,
    fixture: PathBuf,
    bin: String,
    /// Kinds not compared: metadata is compared separately; with split debuginfo, objects and
    /// binaries name .dwo files by session, so two clean builds differ too.
    skip: Vec<&'static str>,
}

struct Build {
    ok: bool,
    ice: bool,
    hang: bool,
    reuse: Vec<String>,
    log: String,
    rmetas: BTreeMap<String, Vec<u8>>,
    exe: Option<String>,
    art: Collected,
}

impl Ctx {
    fn build(&self, src: &Path, target: &Path) -> Build {
        let a = &self.args;
        let mut cmd = Command::new("cargo");
        cmd.arg(format!("+{}", a.toolchain))
            .args([if a.check { "check" } else { "build" }, "--workspace", "--offline", "-j", "4", "--target-dir"])
            .arg(target)
            .arg("--message-format=json-render-diagnostics")
            .current_dir(src)
            .env("RUSTC", &a.rustc)
            .env("RUSTC_WRAPPER", "")
            .env("CARGO_INCREMENTAL", "1")
            .env("RUSTFLAGS", &a.rustflags)
            .env("CARGO_TERM_COLOR", "never");
        if let Some(t) = &a.target {
            cmd.args(["--target", t]);
        }
        if !a.no_verify_reuse {
            cmd.env("RUSTC_VERIFY_REUSE", "1").env("RUSTC_REPORT_UNTRACKED", "1");
        }
        let r = cargo::run_group(cmd, Some(Duration::from_secs(a.timeout))).unwrap_or_else(|e| cargo::Run {
            ok: false,
            stdout: String::new(),
            stderr: format!("cargo did not start: {e}"),
            hang: false,
        });
        let (mut rmetas, mut exe) = (BTreeMap::new(), None);
        for msg in messages(&r.stdout) {
            if msg.reason != "compiler-artifact" {
                continue;
            }
            for f in &msg.filenames {
                if f.ends_with(".rmeta") {
                    rmetas.insert(relative(f, target), std::fs::read(f).unwrap_or_default());
                } else if f.ends_with(".so") && msg.target.as_ref().is_some_and(|t| t.kind.iter().any(|k| k == "proc-macro")) {
                    // A proc macro's metadata is in the .rustc section of its shared library.
                    let section = Command::new("objcopy")
                        .args(["--dump-section", ".rustc=/dev/stdout", f, "/dev/null"])
                        .output()
                        .map(|o| o.stdout)
                        .unwrap_or_default();
                    rmetas.insert(relative(f, target) + ":.rustc", section);
                }
            }
            if let Some(e) = &msg.executable
                && msg.target_name() == self.bin
            {
                exe = Some(e.clone());
            }
        }
        let log = r.stderr;
        Build {
            ok: r.ok && !r.hang,
            ice: cargo::is_ice(&log),
            hang: r.hang,
            reuse: cargo::reuse_checks(&log),
            art: artifacts::collect(&r.stdout, target),
            rmetas,
            exe,
            log,
        }
    }
}

/// Reports seen since the reuse check exists and judged benign (docs/shadow-mode.md): reused
/// codegen units differing only in debuginfo at the end of the file, and constant allocations
/// shared differently between evaluations in different typing modes.
fn known(kind: &str, detail: &[String]) -> bool {
    kind == "verify-reuse"
        && detail.iter().all(|d| d == "codegen unit" || d.starts_with("allocation sharing eval_to_const_value_raw"))
}

fn run_exe(exe: Option<&str>) -> Value {
    let Some(exe) = exe else { return Value::Null };
    match mirth_lab::rustc::run_command(Command::new(exe), Duration::from_secs(30)) {
        Ok(f) => match f.exit {
            mirth_lab::rustc::Exit::Timeout => json!(["timeout", ""]),
            mirth_lab::rustc::Exit::Code(c) => json!([c, tail(&f.stdout_text(), 2000)]),
            // Python's returncode for a signal is its negation.
            mirth_lab::rustc::Exit::Signal(s) => json!([-s, tail(&f.stdout_text(), 2000)]),
        },
        Err(_) => Value::Null,
    }
}

fn unified_diff(old: &str, new: &str, rel: &str) -> String {
    similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .header(&format!("a/{rel}"), &format!("b/{rel}"))
        .to_string()
}

fn rs_files(src: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = walkdir::WalkDir::new(src)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || e.file_name() != "target")
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "rs"))
        .map(|e| e.into_path())
        .collect();
    v.sort();
    v
}

fn tar_gz(src: &Path, out: &Path) -> std::io::Result<()> {
    let f = std::fs::File::create(out)?;
    let mut t = tar::Builder::new(flate2::write::GzEncoder::new(f, flate2::Compression::default()));
    t.follow_symlinks(false);
    t.append_dir_all(".", src)?;
    t.into_inner()?.finish()?;
    Ok(())
}

fn rmtree(p: &Path) {
    let _ = std::fs::remove_dir_all(p);
}

struct Worker<'a> {
    ctx: &'a Ctx,
    k: u64,
    home: PathBuf,
    src: PathBuf,
    target: PathBuf,
    inc_target: PathBuf,
    findings: PathBuf,
    stats: Stats,
    kept: BTreeMap<String, usize>,
    history: Vec<Value>,
    previous: BTreeMap<String, Vec<u8>>,
}

#[derive(serde::Serialize, Default)]
struct Stats {
    edits: u64,
    built: u64,
    failed: u64,
    compared: u64,
    findings: BTreeMap<String, u64>,
    secs: f64,
    by_edit: BTreeMap<String, [u64; 2]>,
}

impl Worker<'_> {
    fn reset(&mut self) -> bool {
        rmtree(&self.home);
        let _ = std::fs::create_dir_all(&self.home);
        if let Err(e) = copy_tree(&self.ctx.fixture, &self.src, &["target", "edits", "edit"], false) {
            let _ = std::fs::write(self.ctx.work.join(format!("error-w{}.log", self.k)), e.to_string());
            return false;
        }
        self.history.clear();
        let b = self.ctx.build(&self.src, &self.target);
        self.previous = b.rmetas;
        if !b.ok {
            let _ = std::fs::write(self.ctx.work.join(format!("error-w{}.log", self.k)), tail(&b.log, 6000));
            return false;
        }
        true
    }

    fn report(&mut self, kind: &str, detail: &[String], inc: &Build, clean: Option<&Build>, extra: Value) {
        *self.stats.findings.entry(kind.to_owned()).or_default() += 1;
        let work = &self.ctx.work;
        if self.ctx.args.pause_on_finding && !known(kind, detail) {
            let paused = json!({"worker": self.k, "edit": self.stats.edits, "kind": kind, "detail": detail});
            let _ = std::fs::write(work.join("PAUSED"), serde_json::to_string_pretty(&paused).unwrap());
            let _ = std::fs::write(work.join("STOP"), "");
        }
        let mut sorted = detail.to_vec();
        sorted.sort();
        let key = format!("{kind}:{}", sorted.join(","));
        let n = self.kept.entry(key).or_default();
        if *n >= self.ctx.args.keep {
            return;
        }
        *n += 1;
        let d = self.findings.join(format!("w{}-{:07}-{kind}", self.k, self.stats.edits));
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("history.json"), serde_json::to_string_pretty(&self.history).unwrap());
        let finding = json!({"kind": kind, "detail": detail, "extra": extra});
        let _ = std::fs::write(d.join("finding.json"), serde_json::to_string_pretty(&finding).unwrap());
        let _ = std::fs::write(d.join("inc.log"), tail(&inc.log, 8000));
        for (name, b) in [("inc", Some(inc)), ("clean", clean)] {
            let lines: Vec<&str> = b.map(|b| b.log.lines().collect()).unwrap_or_default();
            let hit = lines.iter().position(|l| {
                l.contains("panicked at")
                    || l.contains("internal compiler error")
                    || l.contains("unexpectedly panicked")
                    || l.contains("interrupted by SIG")
            });
            if let Some(h) = hit {
                let excerpt = lines[h.saturating_sub(5)..(h + 60).min(lines.len())].join("\n");
                let _ = std::fs::write(d.join(format!("{name}-ice.txt")), excerpt);
            }
        }
        let _ = std::fs::write(d.join("clean.log"), clean.map_or("", |c| tail(&c.log, 8000)));
        for rel in detail {
            let name = Path::new(rel).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if let Some(v) = inc.rmetas.get(rel) {
                let _ = std::fs::write(d.join(format!("{name}.inc")), v);
            }
            if let Some(v) = clean.and_then(|c| c.rmetas.get(rel)) {
                let _ = std::fs::write(d.join(format!("{name}.clean")), v);
            }
        }
        let _ = tar_gz(&self.src, &d.join("src.tar.gz"));
    }

    fn write_stats(&self) {
        let _ = std::fs::write(
            self.ctx.work.join(format!("stats-w{}.json", self.k)),
            serde_json::to_string(&self.stats).unwrap(),
        );
    }

    fn run(&mut self) {
        let _ = std::fs::create_dir_all(&self.findings);
        if !self.reset() {
            return;
        }
        let a = &self.ctx.args;
        let mut rng = StdRng::seed_from_u64(a.seed * 1000 + self.k);
        let started = Instant::now();
        let stop = self.ctx.work.join("STOP");
        while self.stats.edits < a.edits && !stop.exists() {
            if cargo::free_bytes(&self.ctx.work) < a.min_free_gb << 30 {
                println!("worker {}: less than {} GB free, stopping", self.k, a.min_free_gb);
                break;
            }
            if self.history.iter().filter(|h| h["kept"] == true).count() >= a.reset && !self.reset() {
                break;
            }
            let n = self.stats.edits;
            self.stats.edits += 1;
            let paths = rs_files(&self.src);
            let Some(path) = paths.choose(&mut rng).cloned() else { break };
            let old = std::fs::read_to_string(&path).unwrap_or_default();
            let (edit, apply) = mutations::pick(&mut rng);
            if matches!(edit, "int_literal" | "str_literal") && path.file_name().is_some_and(|f| f == "build.rs") {
                // Cargo keeps stale OUT_DIR files, so renaming a generated file splits the
                // builds, and a changed number can make the build script loop forever.
                continue;
            }
            let new = match apply(&old, &mut rng, n as usize) {
                Some(new) if new != old => new,
                _ => continue,
            };
            let _ = std::fs::write(&path, &new);
            let rel = path.strip_prefix(&self.src).unwrap().to_string_lossy().into_owned();
            let diff = unified_diff(&old, &new, &rel);
            let inc = self.ctx.build(&self.src, &self.target);
            cargo::note_untracked(&self.ctx.work.join("untracked.txt"), &cargo::untracked_reads(&inc.log));
            self.history.push(json!({"edit": edit, "file": rel, "diff": diff, "before": old, "after": new, "kept": inc.ok}));
            self.stats.by_edit.entry(edit.to_owned()).or_default()[0] += 1;
            if inc.ice {
                self.report("ICE", &[], &inc, None, Value::Null);
            }
            if inc.hang {
                self.report("hang", &[], &inc, None, Value::Null);
            }
            if !inc.reuse.is_empty() {
                let lines: Vec<String> = inc
                    .log
                    .lines()
                    .filter(|l| l.starts_with("rustc-verify-reuse"))
                    .take(20)
                    .map(|l| l.chars().take(3000).collect())
                    .collect();
                let reuse = inc.reuse.clone();
                self.report("verify-reuse", &reuse, &inc, None, json!(lines));
            }
            if !inc.ok {
                self.stats.failed += 1;
                let _ = std::fs::write(&path, &old);
                self.history.push(json!({"edit": "revert", "file": rel, "diff": "", "kept": false}));
                continue;
            }
            self.stats.by_edit.get_mut(edit).unwrap()[1] += 1;
            self.stats.built += 1;
            self.compare(inc);
            if self.stats.edits % 20 == 0 {
                self.stats.secs = started.elapsed().as_secs_f64();
                self.write_stats();
            }
        }
        self.stats.secs = started.elapsed().as_secs_f64();
        self.write_stats();
    }

    /// The clean build at the same path, and the comparisons.
    fn compare(&mut self, inc: Build) {
        let (target, inc_target) = (self.target.clone(), self.inc_target.clone());
        rmtree(&inc_target);
        let _ = std::fs::rename(&target, &inc_target);
        let clean = self.ctx.build(&self.src, &target);
        if clean.ice {
            self.report("ICE", &["clean".into()], &inc, Some(&clean), Value::Null);
        }
        if clean.hang {
            self.report("hang", &["clean".into()], &inc, Some(&clean), Value::Null);
        }
        if !clean.ok {
            self.report("split", &[], &inc, Some(&clean), Value::Null);
        } else {
            self.stats.compared += 1;
            let skip = &self.ctx.skip;
            let rmeta_differ = |a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>| -> Vec<String> {
                let keys: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
                keys.into_iter().filter(|r| a.get(*r) != b.get(*r)).cloned().collect()
            };
            let mut differ = rmeta_differ(&inc.rmetas, &clean.rmetas);
            let mut others: BTreeMap<&str, Vec<String>> =
                artifacts::compare(&inc.art, &clean.art).into_iter().filter(|(k, _)| !skip.contains(k)).collect();
            if !differ.is_empty() || !others.is_empty() {
                // Build clean again: if two clean builds differ, the difference is
                // nondeterminism (P5), not incremental reuse.
                let mut p5: Vec<String> = Vec::new();
                let mut again_ok = false;
                let mut again = None;
                for _ in 0..self.ctx.args.p5_builds {
                    rmtree(&target);
                    let b = self.ctx.build(&self.src, &target);
                    p5 = rmeta_differ(&clean.rmetas, &b.rmetas);
                    p5.extend(
                        artifacts::compare(&clean.art, &b.art)
                            .into_iter()
                            .filter(|(k, _)| !skip.contains(k))
                            .map(|(k, v)| format!("{k}: {}", v[0])),
                    );
                    again_ok = b.ok;
                    again = Some(b);
                    if !p5.is_empty() || !again_ok {
                        break;
                    }
                }
                if again_ok && !p5.is_empty() {
                    p5.truncate(10);
                    self.report("P5", &p5, &clean, again.as_ref(), Value::Null);
                    differ.clear();
                    others.clear();
                }
            }
            // Known: metadata reused unchanged from the previous session although a source file
            // changed (its hash and length in the source map are stale).
            let stale = differ.iter().any(|r| self.previous.get(r).is_some_and(|p| Some(p) == inc.rmetas.get(r)));
            if !differ.is_empty() && stale {
                *self.stats.findings.entry("P6-stale-reuse".into()).or_default() += 1;
            } else if !differ.is_empty() {
                self.report("P6", &differ, &inc, Some(&clean), Value::Null);
            }
            // More oracles: object code in the rlibs, the binary, and the diagnostics.
            for (kind, detail) in &others {
                self.report(kind, &detail[..detail.len().min(10)], &inc, Some(&clean), Value::Null);
            }
            let inc_exe = inc.exe.as_ref().map(|e| e.replace(&*target.to_string_lossy(), &inc_target.to_string_lossy()));
            let ra = run_exe(inc_exe.as_deref());
            let rb = run_exe(clean.exe.as_deref());
            if ra != rb {
                self.report("run", &[], &inc, Some(&clean), json!({"inc": ra, "clean": rb}));
            }
        }
        rmtree(&target);
        let _ = std::fs::rename(&inc_target, &target);
        self.previous = inc.rmetas;
    }
}

static SPLIT_DEBUGINFO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-Csplit-debuginfo=(packed|unpacked)").unwrap());

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let work = std::fs::canonicalize(&args.work)?;
    let fixture = std::fs::canonicalize(&args.fixture)?;
    for f in ["STOP", "PAUSED"] {
        let _ = std::fs::remove_file(work.join(f));
    }
    let mut skip = vec!["rmeta"];
    if SPLIT_DEBUGINFO.is_match(&args.rustflags) {
        skip.extend(["rlib", "exe"]);
    }
    let ctx = Ctx {
        bin: fixture.file_name().unwrap().to_string_lossy().into_owned(),
        args: args.clone(),
        work: work.clone(),
        fixture,
        skip,
    };
    let totals: Vec<(u64, u64, u64)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..args.workers)
            .map(|k| {
                let ctx = &ctx;
                s.spawn(move || {
                    let home = ctx.work.join(format!("w{k}"));
                    let mut w = Worker {
                        ctx,
                        k,
                        src: home.join("src"),
                        target: home.join("target"),
                        inc_target: home.join("target-inc"),
                        findings: ctx.work.join("findings"),
                        home,
                        stats: Stats::default(),
                        kept: BTreeMap::new(),
                        history: Vec::new(),
                        previous: BTreeMap::new(),
                    };
                    w.run();
                    (w.stats.edits, w.stats.built, w.stats.compared)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    let sum = |f: fn(&(u64, u64, u64)) -> u64| totals.iter().map(f).sum::<u64>();
    println!("{{\"edits\": {}, \"built\": {}, \"compared\": {}}}", sum(|t| t.0), sum(|t| t.1), sum(|t| t.2));
    Ok(ExitCode::SUCCESS)
}

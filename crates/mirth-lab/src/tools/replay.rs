//! Replay a crate's git history through incremental compilation.
//!
//! For each first-parent commit, oldest first, the workspace is checked out and built
//! incrementally on top of the previous commit's build, then built again from scratch, and the
//! two are compared:
//!
//!   P6     every .rmeta Cargo reports for a workspace member is identical
//!   rlib   every rlib's members are identical, object code included
//!   diag   both builds printed the same diagnostics
//!   reuse  the compiler's own check of what it reused (RUSTC_VERIFY_REUSE,
//!          docs/hunt/verify-reuse.patch) found nothing stale
//!   ICE    neither build crashed the compiler
//!   split  both builds succeed or both fail
//!
//! Registry dependencies are not compiled incrementally by Cargo, so the clean build starts
//! from a copy of the incremental target directory with the workspace members and the
//! incremental cache removed, and only the members are built again. Both builds use the same
//! target directory path, since Cargo derives a crate's identity from paths.
//!
//! Writes <work>/results.jsonl, one line per commit, and keeps the logs of every problem and
//! both .rmeta files of the first few differences per crate in <work>/findings.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use serde_json::json;

use mirth_lab::cargo::{self, Collected, copy_tree, messages, relative, tail};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: String,
    #[arg(long)]
    repo: String,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value_t = 2000)]
    commits: usize,
    /// For cargo.
    #[arg(long, default_value = "nightly-2026-10-06")]
    toolchain: String,
    #[arg(long, default_value = "4")]
    jobs: String,
    /// Differences kept per crate.
    #[arg(long, default_value_t = 3)]
    keep: usize,
    /// First commit index to replay.
    #[arg(long = "from", default_value_t = 0)]
    start: usize,
    /// Last commit index to replay.
    #[arg(long = "to")]
    end: Option<usize>,
    /// More flags for every build, such as -Copt-level=2.
    #[arg(long, default_value = "", allow_hyphen_values = true)]
    rustflags: String,
    /// Do not set RUSTC_VERIFY_REUSE (needs a compiler with docs/hunt/verify-reuse.patch).
    #[arg(long)]
    no_verify_reuse: bool,
}

struct Ctx<'a> {
    args: &'a Args,
    src: PathBuf,
}

struct Build {
    ok: bool,
    ice: bool,
    secs: f64,
    log: String,
    reuse: Vec<String>,
    untracked: BTreeSet<String>,
    rmetas: BTreeMap<String, Vec<u8>>,
    fresh: Vec<String>,
    art: Collected,
}

impl Ctx<'_> {
    fn cmd(&self, program: &str) -> Command {
        let mut c = Command::new(program);
        c.current_dir(&self.src)
            .env("RUSTC", &self.args.rustc)
            .env("RUSTC_WRAPPER", "")
            .env("CARGO_INCREMENTAL", "1")
            // A commit that denies warnings would otherwise stop building with a newer compiler.
            .env("RUSTFLAGS", format!("--cap-lints=warn {}", self.args.rustflags).trim())
            .env("CARGO_TERM_COLOR", "never");
        if !self.args.no_verify_reuse {
            c.env("RUSTC_VERIFY_REUSE", "1").env("RUSTC_REPORT_UNTRACKED", "1");
        }
        if program == "cargo" {
            c.arg(format!("+{}", self.args.toolchain));
        }
        c
    }

    fn run(&self, program: &str, args: &[&str]) -> (bool, String) {
        match self.cmd(program).args(args).output() {
            Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned()),
            Err(_) => (false, String::new()),
        }
    }

    /// Every package built from a path: workspace members and path dependencies.
    fn members(&self) -> Vec<String> {
        let (mut ok, mut out) = self.run("cargo", &["metadata", "--format-version", "1"]);
        if !ok {
            (ok, out) = self.run("cargo", &["metadata", "--no-deps", "--format-version", "1"]);
            if !ok {
                return Vec::new();
            }
        }
        let meta: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
        let names: BTreeSet<String> = meta["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p.get("source").is_none_or(|s| s.is_null()))
            .filter_map(|p| p["name"].as_str().map(str::to_owned))
            .collect();
        names.into_iter().collect()
    }

    /// Build, and collect the .rmeta files Cargo reports for workspace members.
    fn build(&self, target: &Path) -> Build {
        let t = Instant::now();
        let mut c = self.cmd("cargo");
        c.args(["build", "--lib", "-j", &self.args.jobs, "--target-dir"])
            .arg(target)
            .arg("--message-format=json-render-diagnostics");
        let r = cargo::run_group(c, None).unwrap_or_else(|e| cargo::Run {
            ok: false,
            stdout: String::new(),
            stderr: format!("cargo did not start: {e}"),
            hang: false,
        });
        let (mut rmetas, mut fresh) = (BTreeMap::new(), Vec::new());
        for msg in messages(&r.stdout) {
            if msg.reason != "compiler-artifact" || !msg.from_path() {
                continue;
            }
            for f in &msg.filenames {
                if f.ends_with(".rmeta") {
                    rmetas.insert(relative(f, target), std::fs::read(f).unwrap_or_default());
                    if msg.fresh {
                        fresh.push(msg.target_name().to_owned());
                    }
                }
            }
        }
        let log = &r.stderr;
        Build {
            ok: r.ok,
            ice: cargo::is_ice(log),
            secs: (t.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            log: tail(log, 6000).to_owned(),
            reuse: log.lines().filter(|l| l.starts_with("rustc-verify-reuse:")).map(|l| l.chars().take(3000).collect()).collect(),
            untracked: cargo::untracked_reads(log),
            rmetas,
            fresh,
            art: cargo::collect(&r.stdout, target),
        }
    }
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let work = std::fs::canonicalize(&args.work)?;
    let (src, target, inc_target) = (work.join("src"), work.join("target"), work.join("target-inc"));
    let findings = work.join("findings");
    std::fs::create_dir_all(&findings)?;
    if !src.exists() {
        let ok = Command::new("git").args(["clone", "-q", &args.repo]).arg(&src).status()?.success();
        anyhow::ensure!(ok, "git clone {} failed", args.repo);
    }
    let ctx = Ctx { args: &args, src: src.clone() };
    let max = format!("--max-count={}", args.commits);
    let mut commits: Vec<String> = ctx.run("git", &["rev-list", "--first-parent", "--reverse", &max, "origin/HEAD"]).1.split_whitespace().map(str::to_owned).collect();
    if commits.is_empty() {
        commits = ctx.run("git", &["rev-list", "--first-parent", "--reverse", &max, "HEAD"]).1.split_whitespace().map(str::to_owned).collect();
    }
    let results = work.join("results.jsonl");
    let done: BTreeSet<String> = std::fs::read_to_string(&results)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v["commit"].as_str().map(str::to_owned))
        .collect();
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
    let keep_dir = |i: usize, commit: &str| -> PathBuf {
        let d = findings.join(format!("{i:05}-{}", &commit[..10.min(commit.len())]));
        let _ = std::fs::create_dir_all(&d);
        d
    };

    for (i, commit) in commits.iter().enumerate() {
        if done.contains(commit) || i < args.start || args.end.is_some_and(|e| i > e) {
            continue;
        }
        ctx.run("git", &["checkout", "-q", "--force", commit]);
        ctx.run("git", &["clean", "-fdxq"]);
        let date = ctx.run("git", &["log", "-1", "--format=%cs", commit]).1.trim().to_owned();
        let names = ctx.members();
        let inc = ctx.build(&target);
        // Reads of untracked state, each listed once in <work>/untracked.txt.
        cargo::note_untracked(&work.join("untracked.txt"), &inc.untracked);

        // The clean build: the same target directory path, starting from a copy with the
        // workspace members and the incremental cache removed.
        let _ = std::fs::remove_dir_all(&inc_target);
        if target.exists() {
            std::fs::rename(&target, &inc_target)?;
            copy_tree(&inc_target, &target, &[], true)?;
        } else {
            std::fs::create_dir_all(&inc_target)?;
        }
        // Cargo refuses to clean a directory it did not create; this one may have been created
        // here when the first build failed early.
        std::fs::create_dir_all(&target)?;
        let tag = target.join("CACHEDIR.TAG");
        if !tag.exists() {
            std::fs::write(&tag, "Signature: 8a477f597d28d172789f06886806bc55\n")?;
        }
        let target_str = target.to_string_lossy().into_owned();
        for name in &names {
            ctx.run("cargo", &["clean", "-p", name, "--target-dir", &target_str]);
        }
        for e in std::fs::read_dir(&target)?.flatten() {
            let inc_dir = e.path().join("incremental");
            if inc_dir.is_dir() {
                std::fs::remove_dir_all(inc_dir)?;
            }
        }
        let clean = ctx.build(&target);

        let (mut problems, mut differ): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        if inc.ice || clean.ice {
            problems.push("ICE".into());
        }
        if inc.ok != clean.ok {
            problems.push("split".into());
        }
        if !inc.reuse.is_empty() {
            // The compiler's own check found something it reused stale.
            problems.push("reuse".into());
            std::fs::write(keep_dir(i, commit).join("reuse.txt"), inc.reuse.join("\n"))?;
        }
        if !clean.fresh.is_empty() {
            // A member the clean build did not compile again would be compared with itself.
            problems.push("stale".into());
        }
        if inc.ok && clean.ok && clean.fresh.is_empty() {
            let (a, b) = (&inc.rmetas, &clean.rmetas);
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            differ = keys.into_iter().filter(|r| a.get(*r) != b.get(*r)).cloned().collect();
            // More oracles: object code in the rlibs and the diagnostics.
            for (kind, detail) in cargo::compare(&inc.art, &clean.art) {
                if kind == "rlib" || kind == "diag" {
                    std::fs::write(keep_dir(i, commit).join(format!("{kind}.txt")), detail.join("\n"))?;
                    problems.push(kind);
                }
            }
            if !differ.is_empty() {
                problems.push("P6".into());
                for rel in &differ {
                    let file = Path::new(rel).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let krate = file.split('-').next().unwrap_or("").to_owned();
                    let n = kept.entry(krate).or_default();
                    if *n < args.keep {
                        *n += 1;
                        let d = keep_dir(i, commit);
                        std::fs::write(d.join(format!("{file}.inc")), a.get(rel).map_or(&[][..], |v| v))?;
                        std::fs::write(d.join(format!("{file}.clean")), b.get(rel).map_or(&[][..], |v| v))?;
                    }
                }
            }
        }
        if !problems.is_empty() {
            let d = keep_dir(i, commit);
            std::fs::write(d.join("inc.log"), &inc.log)?;
            std::fs::write(d.join("clean.log"), &clean.log)?;
        }

        // Continue incrementally from the incremental build.
        let _ = std::fs::remove_dir_all(&target);
        std::fs::rename(&inc_target, &target)?;
        let record = json!({
            "i": i, "commit": commit, "date": date, "members": names,
            "inc": {"ok": inc.ok, "ice": inc.ice, "secs": inc.secs},
            "clean": {"ok": clean.ok, "ice": clean.ice, "secs": clean.secs},
            "compared": inc.rmetas.len(), "differ": differ, "problems": problems,
        });
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&results)?;
        writeln!(f, "{record}")?;
        let status = if inc.ok && clean.ok {
            "both ok".to_owned()
        } else {
            format!("inc {}, clean {}", if inc.ok { "ok" } else { "failed" }, if clean.ok { "ok" } else { "failed" })
        };
        println!(
            "{i:5} {date} {} {status}, {} compared, {:.1}s/{:.1}s {}",
            &commit[..10.min(commit.len())],
            inc.rmetas.len(),
            inc.secs,
            clean.secs,
            problems.join(" ")
        );
    }
    Ok(ExitCode::SUCCESS)
}

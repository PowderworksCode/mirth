//! Audit rustc's [UNTRACKED] options for stale incremental reuse.
//!
//! An option rustc marks [UNTRACKED] is left out of the dependency-tracking hash, so changing
//! it between incremental sessions reuses the previous session's results. That is only correct
//! if the option cannot change them. For each untracked option that takes no value or a
//! boolean, this builds a crate incrementally without it, then again with it, and compares the
//! result with a clean build that has it: the .rmeta, each .rlib member (object code, with the
//! incremental session suffix removed from names), the diagnostics, and the files written. A
//! difference means the option changes output that incremental compilation reuses: it should
//! be tracked, or the reuse checked.
//!
//! With ARGs, audits those arguments instead of the options found in the checkout's
//! compiler/rustc_session/src/options.rs. The crate should have code of its own for codegen
//! options to change (non-generic functions) and a warning or two for diagnostic options to
//! change. Exits 1 when any option differs.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;

use regex::Regex;

use mirth_lab::artifacts::normalized_rlib;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// A rust checkout, to read the untracked options from.
    #[arg(long)]
    source: Option<PathBuf>,
    /// The crate root to build, as a library.
    #[arg(long = "crate")]
    krate: PathBuf,
    #[arg(long, default_value = "audited")]
    crate_name: String,
    #[arg(long, default_value = "2021")]
    edition: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

// Options that stop compilation or change how arguments are read.
const SKIP: &[&str] = &["-Chelp", "-Zhelp", "-Zno-analysis", "-Zparse-crate-root-only=yes", "-Zshell-argfiles=yes"];

static UNTRACKED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s{4}(\w+):\s*([^=\n]+?)\s*=\s*\(([^,]*),\s*(parse_\w+),\s*\[UNTRACKED\]").unwrap());
static CODEGEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"options! \{\s*CodegenOptions,").unwrap());
static UNSTABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"options! \{\s*UnstableOptions,").unwrap());

fn untracked_boolean_options(source: &Path) -> anyhow::Result<Vec<String>> {
    let text = std::fs::read_to_string(source.join("compiler/rustc_session/src/options.rs"))?;
    let codegen = CODEGEN.find(&text).map_or(0, |m| m.start());
    let unstable = UNSTABLE.find(&text).map_or(0, |m| m.start());
    let mut found = Vec::new();
    for c in UNTRACKED.captures_iter(&text) {
        let start = c.get(0).unwrap().start();
        let (name, default, parser) = (&c[1], c[3].trim(), &c[4]);
        if !["parse_bool", "parse_no_value", "parse_opt_bool"].contains(&parser) || start < codegen {
            continue;
        }
        let group = if start > unstable { "Z" } else { "C" };
        let mut flag = format!("-{group}{}", name.replace('_', "-"));
        if parser != "parse_no_value" {
            flag += if default == "true" || default == "Some(true)" { "=no" } else { "=yes" };
        }
        if !SKIP.contains(&flag.as_str()) {
            found.push(flag);
        }
    }
    Ok(found)
}

/// One build, always from the same working directory, which rustc records.
fn build(args: &Args, krate: &Path, work: &Path, incremental: &str, out: &str, extra: &[&str]) -> (i32, Vec<String>) {
    let _ = std::fs::create_dir_all(work.join(out));
    let r = Command::new(&args.rustc)
        .args(["--edition", &args.edition, "--crate-type", "lib", "--crate-name", &args.crate_name, "--emit=metadata,link"])
        .arg(format!("-Cincremental={}", work.join(incremental).display()))
        .arg("--out-dir")
        .arg(work.join(out))
        .arg(krate)
        .args(extra)
        .current_dir(work)
        .output();
    match r {
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let mut diagnostics: Vec<String> =
                err.lines().filter(|l| l.starts_with("warning") || l.starts_with("error") || l.contains("-->")).map(str::to_owned).collect();
            diagnostics.sort();
            (o.status.code().unwrap_or(-1), diagnostics)
        }
        Err(_) => (-1, Vec::new()),
    }
}

fn listing(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
}

fn audit(args: &Args, krate: &Path, flag: &str) -> Vec<String> {
    let work = tempfile::Builder::new().prefix("audit-").tempdir().expect("scratch");
    let w = work.path();
    let extra: Vec<&str> = if flag.is_empty() { vec![] } else { vec![flag] };
    let (rc0, _) = build(args, krate, w, "i", "o1", &[]);
    let (rc1, diag_inc) = build(args, krate, w, "i", "o1", &extra);
    let (rc2, diag_clean) = build(args, krate, w, "j", "o2", &extra);
    if rc0 != 0 || rc2 != 0 {
        return vec![format!("the crate does not build (exit {rc0} without the option, {rc2} with it)")];
    }
    let name = &args.crate_name;
    let mut problems = Vec::new();
    if rc1 != rc2 {
        problems.push(format!("the incremental rebuild exits {rc1}, a clean build {rc2}"));
    }
    if std::fs::read(w.join(format!("o1/lib{name}.rmeta"))).ok() != std::fs::read(w.join(format!("o2/lib{name}.rmeta"))).ok() {
        problems.push("metadata".into());
    }
    let a = normalized_rlib(&w.join(format!("o1/lib{name}.rlib")));
    let b = normalized_rlib(&w.join(format!("o2/lib{name}.rlib")));
    let members: BTreeSet<&String> = a.keys().chain(b.keys()).filter(|m| a.get(*m) != b.get(*m)).collect();
    if !members.is_empty() {
        problems.push(format!("object code ({} rlib members differ or exist on one side)", members.len()));
    }
    if diag_inc != diag_clean {
        problems.push(format!("diagnostics ({} lines incrementally, {} clean)", diag_inc.len(), diag_clean.len()));
    }
    let o1 = listing(&w.join("o1"));
    let missing = listing(&w.join("o2")).difference(&o1).count();
    if missing > 0 {
        problems.push(format!("{missing} files only a clean build writes"));
    }
    problems
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let krate = std::fs::canonicalize(&args.krate)?;
    let flags = if !args.args.is_empty() {
        args.args.clone()
    } else {
        let source = args.source.as_ref().ok_or_else(|| anyhow::anyhow!("--source or ARGs needed"))?;
        untracked_boolean_options(source)?
    };
    let show = |p: &[String]| if p.is_empty() { "same".to_owned() } else { p.join("; ") };
    let control = audit(&args, &krate, "");
    println!("{:40} {}", "(control: no option)", show(&control));
    if !control.is_empty() {
        eprintln!("the control differs: incremental and clean builds disagree without any option");
        return Ok(ExitCode::from(1));
    }
    let mut any = false;
    for flag in &flags {
        let r = audit(&args, &krate, flag);
        println!("{flag:40} {}", show(&r));
        any |= !r.is_empty();
    }
    Ok(if any { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

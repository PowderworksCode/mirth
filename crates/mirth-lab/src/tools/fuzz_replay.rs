//! Replay a fuzz finding: apply its edits to the pristine fixture in order, with an incremental
//! build after each, then compare with a clean build.
//!
//! Prints which .rmeta files differ, and keeps the compiler's output of the last incremental
//! build and the clean build as inc.log and clean.log. --upto replays only the first N edits
//! that were kept, to find where the difference appears.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::Deserialize;

use mirth_lab::cargo::{self, copy_tree, messages, relative};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: String,
    #[arg(long)]
    fixture: PathBuf,
    #[arg(long)]
    finding: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long)]
    upto: Option<usize>,
    #[arg(long, default_value = "nightly-2026-10-06")]
    toolchain: String,
    #[arg(long, default_value = "-Zincremental-verify-ich", allow_hyphen_values = true)]
    rustflags: String,
    #[arg(long)]
    quiet: bool,
}

#[derive(Deserialize)]
struct Step {
    edit: String,
    file: String,
    #[serde(default)]
    before: String,
    #[serde(default)]
    after: String,
    kept: bool,
}

/// Whether it built, the .rmeta files by path, and the compiler's output.
fn build(args: &Args, src: &Path, target: &Path) -> (bool, BTreeMap<String, Vec<u8>>, String) {
    let mut cmd = Command::new("cargo");
    cmd.arg(format!("+{}", args.toolchain))
        .args(["build", "--workspace", "--offline", "-j", "4", "--target-dir"])
        .arg(target)
        .arg("--message-format=json-render-diagnostics")
        .current_dir(src)
        .env("RUSTC", &args.rustc)
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_INCREMENTAL", "1")
        .env("RUSTFLAGS", &args.rustflags);
    let Ok(r) = cargo::run_group(cmd, None) else { return (false, BTreeMap::new(), "cargo did not start".into()) };
    let rmetas = messages(&r.stdout)
        .filter(|m| m.reason == "compiler-artifact")
        .flat_map(|m| m.filenames)
        .filter(|f| f.ends_with(".rmeta"))
        .map(|f| (relative(&f, target), std::fs::read(&f).unwrap_or_default()))
        .collect();
    (r.ok, rmetas, r.stderr)
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let _ = std::fs::remove_dir_all(&args.work);
    std::fs::create_dir_all(&args.work)?;
    let work = std::fs::canonicalize(&args.work)?;
    let (src, target, inc_target) = (work.join("src"), work.join("target"), work.join("target-inc"));
    copy_tree(&std::fs::canonicalize(&args.fixture)?, &src, &["target", "edits", "edit"], false)?;
    build(&args, &src, &target);

    let history: Vec<Step> = serde_json::from_str(&std::fs::read_to_string(args.finding.join("history.json"))?)?;
    let mut kept = 0;
    let mut last: Option<&Step> = None;
    for step in &history {
        if step.edit == "revert" {
            if let Some(l) = last {
                std::fs::write(src.join(&l.file), &l.before)?;
            }
        } else {
            if args.upto.is_some_and(|u| step.kept && kept >= u) {
                break;
            }
            std::fs::write(src.join(&step.file), &step.after)?;
            last = Some(step);
            kept += step.kept as usize;
        }
        let (ok, _, _) = build(&args, &src, &target);
        if !args.quiet {
            println!("{:18} {:24} {}", step.edit, step.file, if ok { "built" } else { "failed" });
        }
    }

    let (ok, inc, log) = build(&args, &src, &target);
    std::fs::write(work.join("inc.log"), log)?;
    std::fs::rename(&target, &inc_target)?;
    let (ok2, clean, log) = build(&args, &src, &target);
    std::fs::write(work.join("clean.log"), log)?;
    let keys: BTreeSet<&String> = inc.keys().chain(clean.keys()).collect();
    let differ: Vec<&String> = keys.into_iter().filter(|r| inc.get(*r) != clean.get(*r)).collect();
    let name = |d: &str| Path::new(d).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let names: Vec<String> = differ.iter().map(|d| name(d)).collect();
    println!(
        "{{\"kept_edits\": {kept}, \"inc_ok\": {ok}, \"clean_ok\": {ok2}, \"differ\": {}}}",
        serde_json::to_string(&names)?.replace("\",\"", "\", \"")
    );
    for d in differ {
        let n = name(d);
        std::fs::write(work.join(format!("{n}.inc")), inc.get(d).map_or(&[][..], |v| v))?;
        std::fs::write(work.join(format!("{n}.clean")), clean.get(d).map_or(&[][..], |v| v))?;
    }
    Ok(ExitCode::SUCCESS)
}

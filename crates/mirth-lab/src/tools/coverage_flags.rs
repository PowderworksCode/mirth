//! Coverage of the compiler across option configurations: build a fixture with a
//! coverage-instrumented rustc (rustc/coverage.toml) once per row of a PICT transitions table,
//! clean with the A options, then rebuilt after one random edit with the B options, each row's
//! rustc processes logging to <out>/row<i>/. Read the result with `mirth-lab coverage`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng as _, SeedableRng};

use super::flag_model::{self, Opt};
use super::flag_walk::{copy_fixture, slice};
use mirth_lab::mutations;
use mirth_lab::rustc::run_command;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// A coverage-instrumented rustc.
    #[arg(long)]
    rustc: String,
    #[arg(long)]
    fixture: PathBuf,
    /// flag-universe's work directory.
    #[arg(long)]
    flags: PathBuf,
    #[arg(long)]
    table: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = "")]
    rows: String,
    #[arg(long, default_value_t = 6)]
    workers: usize,
    #[arg(long, default_value = "nightly-2026-10-06")]
    toolchain: String,
}

/// One random edit somewhere in the fixture, build scripts aside.
fn edit(src: &Path, rng: &mut StdRng) {
    let mut paths: Vec<PathBuf> = walkdir::WalkDir::new(src)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs") && !p.strip_prefix(src).unwrap().components().any(|c| c.as_os_str() == "target"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        return;
    }
    for _ in 0..20 {
        let path = &paths[rng.random_range(..paths.len())];
        let (_, f) = mutations::pick(rng);
        if path.file_name().is_some_and(|n| n == "build.rs") {
            continue;
        }
        let Ok(old) = std::fs::read_to_string(path) else { continue };
        if let Some(new) = f(&old, rng, 0) {
            let _ = std::fs::write(path, new);
            break;
        }
    }
}

fn row(args: &Args, opts: &std::collections::BTreeMap<String, Opt>, out: &Path, i: usize, row: &[(String, String)]) -> String {
    let logs = out.join(format!("row{i}"));
    if logs.exists() {
        return "done before".into();
    }
    let work = out.join(format!("work{i}"));
    let _ = std::fs::remove_dir_all(&work);
    let src = work.join("s");
    if let Err(e) = copy_fixture(&args.fixture, &src) {
        return format!("cannot copy the fixture: {e}");
    }
    let mut results = Vec::new();
    for side in ["A", "B"] {
        if side == "B" {
            edit(&src, &mut StdRng::seed_from_u64(i as u64));
        }
        let mut cmd = Command::new("cargo");
        cmd.arg(format!("+{}", args.toolchain))
            .args(["build", "--workspace", "--offline", "-j", "4", "--target", "x86_64-unknown-linux-gnu", "--target-dir"])
            .arg(work.join("t"))
            .current_dir(&src)
            .env("RUSTC", &args.rustc)
            .env("RUSTC_WRAPPER", "")
            .env("CARGO_INCREMENTAL", "1")
            .env("RUSTFLAGS", flag_model::row_flags(row, Some(side), opts).join(" "))
            .env("MIRTH_OUT", work.join("logs"));
        let ok = run_command(cmd, Duration::from_secs(1800)).is_ok_and(|f| f.success());
        results.push(if ok { "ok" } else { "failed" });
    }
    let _ = std::fs::create_dir_all(work.join("logs"));
    let _ = std::fs::rename(work.join("logs"), &logs);
    let _ = std::fs::remove_dir_all(&work);
    results.join(" ")
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.out)?;
    let out = std::fs::canonicalize(&args.out)?;
    let opts = flag_model::options(&args.flags)?;
    let rows = flag_model::table(&args.table)?;
    let idx = slice(&args.rows, rows.len());
    // Rows in order, a worker taking the next one when it is free.
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..args.workers.max(1) {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::SeqCst);
                    let Some(&i) = idx.get(k) else { break };
                    println!("row {i}: {}", row(&args, &opts, &out, i, &rows[i]));
                }
            });
        }
    });
    Ok(ExitCode::SUCCESS)
}

//! Run the fuzzer under option configurations: for each chosen row of a PICT table (the B side
//! of a `flag-model --transitions --cargo` model), fuzz the fixture with those options in
//! RUSTFLAGS for a number of edits. Stops at the first finding (`mirth-lab fuzz
//! --pause-on-finding`, exit 3); rerunning resumes after the rows already done.
//!
//! Writes <dir>/row<i>/ (the fuzzer's work directory) and <dir>/rows.jsonl (one line per
//! finished row: options, the fuzzer's totals).

use std::collections::HashSet;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use serde::{Deserialize, Serialize};

use super::flag_model;
use super::flag_walk::slice;

#[derive(clap::Args, Debug)]
pub struct Args {
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
    work: PathBuf,
    #[arg(long, default_value = "")]
    rows: String,
    /// Per row, over all workers.
    #[arg(long, default_value_t = 200)]
    edits: usize,
    #[arg(long, default_value_t = 8)]
    workers: usize,
}

#[derive(Serialize, Deserialize)]
struct Done {
    row: usize,
    flags: Vec<String>,
    total: String,
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let work = std::fs::canonicalize(&args.work)?;
    let opts = flag_model::options(&args.flags)?;
    let rows = flag_model::table(&args.table)?;
    let log = work.join("rows.jsonl");
    let done: HashSet<usize> = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()?["row"].as_u64().map(|r| r as usize))
        .collect();
    let me = std::env::current_exe()?;
    for i in slice(&args.rows, rows.len()) {
        if done.contains(&i) {
            continue;
        }
        let flags = flag_model::row_flags(&rows[i], Some("B"), &opts);
        let w = work.join(format!("row{i}"));
        let out = Command::new(&me)
            .arg("fuzz")
            .args(["--rustc", &args.rustc, "--fixture"])
            .arg(&args.fixture)
            .arg("--work")
            .arg(&w)
            .args(["--workers", &args.workers.to_string()])
            .args(["--edits", &(args.edits / args.workers.max(1)).max(1).to_string()])
            .args(["--seed", &i.to_string()])
            .args(["--rustflags", &flags.join(" ")])
            .args(["--target", "x86_64-unknown-linux-gnu", "--pause-on-finding"])
            .output()?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let total = stdout.trim().lines().last().unwrap_or("").to_owned();
        let paused = std::fs::read_to_string(w.join("PAUSED")).ok();
        match &paused {
            Some(p) => println!("row {i}: {total} PAUSED {p}"),
            None => println!("row {i}: {total}"),
        }
        if paused.is_some() {
            return Ok(ExitCode::from(3)); // not recorded as done: rerun after patching to do this row again
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&log)?;
        writeln!(f, "{}", serde_json::to_string(&Done { row: i, flags: flags[1..].to_vec(), total })?)?;
    }
    Ok(ExitCode::SUCCESS)
}

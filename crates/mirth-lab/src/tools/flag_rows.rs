//! Compile a trivial crate once per row of a PICT table made from flag-model's model, and count
//! the rows rustc rejects, grouped by first error.
//!
//! Tables from a --transitions model are not supported.

use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

use rayon::prelude::*;

use super::flag_model;
use mirth_lab::rustc::run_command;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// flag-universe's work directory (scratch directories go here too).
    work: PathBuf,
    table: PathBuf,
    rustc: PathBuf,
    /// Extra rustc arguments, e.g. --emit=metadata.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    extra: Vec<String>,
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let opts = flag_model::options(&args.work)?;
    let rows = flag_model::table(&args.table)?;
    let pool = rayon::ThreadPoolBuilder::new().num_threads(10).build()?;
    let res: Vec<(bool, String, usize)> = pool.install(|| {
        rows.par_iter()
            .map(|row| {
                let d = tempfile::tempdir_in(&args.work).expect("scratch");
                let _ = std::fs::write(d.path().join("lib.rs"), "pub fn f(x: u32) -> u32 { x.wrapping_mul(3) }\n");
                let a = flag_model::row_flags(row, None, &opts);
                let mut cmd = Command::new(&args.rustc);
                cmd.args(["--edition", "2021", "--crate-type", "lib"])
                    .args(&args.extra)
                    .arg("-o")
                    .arg(d.path().join("out"))
                    .args(&a)
                    .arg(d.path().join("lib.rs"))
                    .current_dir(d.path());
                match run_command(cmd, Duration::from_secs(300)) {
                    Ok(f) => {
                        let err = f.stderr_text().lines().find(|l| l.starts_with("error")).unwrap_or("").to_owned();
                        (f.success(), err, a.len() - 1)
                    }
                    Err(e) => (false, e.to_string(), a.len() - 1),
                }
            })
            .collect()
    });
    let bad: Vec<&String> = res.iter().filter(|r| !r.0).map(|r| &r.1).collect();
    let avg = res.iter().map(|r| r.2).sum::<usize>() as f64 / res.len().max(1) as f64;
    println!("{} rows, {} rejected, {avg:.0} options per row on average", rows.len(), bad.len());
    // Most common first; ties in first-seen order, as Counter.most_common.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for e in bad {
        let k: String = e.chars().take(110).collect();
        match counts.iter_mut().find(|(x, _)| *x == k) {
            Some(c) => c.1 += 1,
            None => counts.push((k, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    for (e, c) in counts {
        println!("{c} {e}");
    }
    Ok(ExitCode::SUCCESS)
}

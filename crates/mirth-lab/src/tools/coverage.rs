//! Which of the compiler's functions ran, from a compiler built with rustc/coverage.toml.
//!
//! The site tables list each instrumented function (`cover` sites: id, crate, path, span); every
//! rustc process run with MIRTH_OUT set writes, at exit, a `V <site>` line for each function it
//! entered. This reads both and prints, per crate, how many functions ran; with --files, per
//! source file; with --unhit, the functions that never ran, in the crates or files matching.
//! Several --logs directories (several runs: fixtures, flags) are combined.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::ExitCode;

use mirth_lab::coverage::{self, to_json_indent};
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    sites: PathBuf,
    #[arg(long, required = true)]
    logs: Vec<PathBuf>,
    #[arg(long)]
    json: Option<PathBuf>,
    /// Also per source file.
    #[arg(long)]
    files: bool,
    /// List the functions that never ran in crates or files matching.
    #[arg(long)]
    unhit: Vec<String>,
}

#[derive(Serialize, Default, Clone, Copy)]
struct Count {
    functions: usize,
    ran: usize,
}

#[derive(Serialize)]
struct Report {
    processes: usize,
    functions: usize,
    ran: usize,
    crates: BTreeMap<String, Count>,
    files: BTreeMap<String, Count>,
    unhit: Vec<String>,
}

/// A span's file: all but its last two `:` fields.
pub fn span_file(span: &str) -> &str {
    span.rsplitn(3, ':').last().unwrap_or(span)
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    // A site listed twice counts once (the last listing).
    let functions: Vec<coverage::Function> =
        coverage::functions(&args.sites).into_iter().map(|f| (f.site.clone(), f)).collect::<BTreeMap<_, _>>().into_values().collect();
    let mut hit = BTreeSet::new();
    let mut processes = 0;
    for d in &args.logs {
        let (h, n) = coverage::hits(d);
        hit.extend(h);
        processes += n;
    }
    let sites: BTreeSet<&str> = functions.iter().map(|f| f.site.as_str()).collect();
    let unknown = hit.iter().filter(|s| !sites.contains(s.as_str())).count();
    let (mut by_crate, mut by_file) = (BTreeMap::<String, Count>::new(), BTreeMap::<String, Count>::new());
    for f in &functions {
        let ran = hit.contains(&f.site) as usize;
        for c in [by_crate.entry(f.krate.clone()).or_default(), by_file.entry(span_file(&f.span).to_owned()).or_default()] {
            c.functions += 1;
            c.ran += ran;
        }
    }
    let total = sites.len();
    let ran = hit.iter().filter(|s| sites.contains(s.as_str())).count();
    print!("{processes} processes; {ran} of {total} functions ran ({:.1}%)", 100.0 * ran as f64 / total.max(1) as f64);
    println!("{}", if unknown > 0 { format!("; {unknown} sites not in the tables") } else { String::new() });
    println!("{:40} {:>7} {:>7} {:>6}", "crate", "ran", "of", "%");
    let ratio = |c: &Count| c.ran as f64 / c.functions as f64;
    let mut crates: Vec<(&String, &Count)> = by_crate.iter().collect();
    crates.sort_by(|a, b| ratio(a.1).total_cmp(&ratio(b.1)));
    for (k, c) in crates {
        println!("{k:40} {:7} {:7} {:6.1}", c.ran, c.functions, 100.0 * ratio(c));
    }
    if args.files {
        println!();
        println!("{:80} {:>6} {:>6}", "file", "ran", "of");
        let mut files: Vec<(&String, &Count)> = by_file.iter().collect();
        files.sort_by(|a, b| ratio(a.1).total_cmp(&ratio(b.1)).then(b.1.functions.cmp(&a.1.functions)));
        for (k, c) in files {
            println!("{k:80} {:6} {:6}", c.ran, c.functions);
        }
    }
    let mut by_span: Vec<&coverage::Function> = functions.iter().collect();
    by_span.sort_by(|a, b| a.span.cmp(&b.span));
    for pattern in &args.unhit {
        println!("\nnever ran, matching '{pattern}':");
        for f in &by_span {
            if !hit.contains(&f.site) && (f.krate.contains(pattern.as_str()) || f.span.contains(pattern.as_str())) {
                println!("  {}  {}", f.path, f.span);
            }
        }
    }
    if let Some(j) = &args.json {
        let mut unhit: Vec<String> = functions.iter().filter(|f| !hit.contains(&f.site)).map(|f| format!("{}\t{}", f.path, f.span)).collect();
        unhit.sort();
        let report = Report { processes, functions: total, ran, crates: by_crate, files: by_file, unhit };
        std::fs::write(j, to_json_indent(&report, 1))?;
    }
    Ok(ExitCode::SUCCESS)
}

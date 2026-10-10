//! Enumerate rustc's -C and -Z options, find which values and pairs of values it accepts, and
//! size covering arrays over them.
//!
//! Each option's domain is its absence plus the values worth trying: `yes`/`no` for a boolean,
//! present for an option without a value, the values its parser's description lists for an
//! enumerated one, two samples for a number. Options taking free-form strings, paths or lists
//! are counted but left out. Every value is tried alone on a trivial crate (`--emit=metadata`,
//! so only option checking and a tiny compilation run), then every pair of accepted values of
//! different options. A pair is "rejected" when rustc fails with the pair but accepts each
//! value alone.
//!
//! Writes <work>/options.json (domains), <work>/singles.json, <work>/pairs.json, and prints the
//! sizes of pairwise covering arrays built greedily over the accepted domains.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;
use std::time::Duration;

use rand::{Rng as _, SeedableRng};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::flag_model::{to_json_indent1, Opt, OrderedMap, Single};
use mirth_lab::rustc::{run_command, Exit};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// A rust checkout, for compiler/rustc_session/src/options.rs.
    #[arg(long)]
    source: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value_t = 8)]
    jobs: usize,
    #[arg(long)]
    skip_pairs: bool,
}

const BOOL: &[&str] = &["parse_bool", "parse_opt_bool"];
const NO_VALUE: &[&str] = &["parse_no_value"];
const NUMBER: &[&str] = &["parse_number", "parse_opt_number"];
const FREE: &[&str] = &[
    "parse_string", "parse_opt_string", "parse_string_push", "parse_opt_pathbuf", "parse_list", "parse_comma_list",
    "parse_opt_comma_list", "parse_ignore", "parse_target_feature", "parse_list_with_polarity", "parse_llvm_module_flag",
    "parse_patchable_function_entry", "parse_autodiff", "parse_offload", "parse_allow_partial_mitigations",
    "parse_deny_partial_mitigations", "parse_rust_version", "parse_unpretty", "parse_passes", "parse_branch_protection",
    "parse_instrument_xray", "parse_linker_features", "parse_link_self_contained", "parse_align", "parse_location_detail",
    "parse_coverage_options", "parse_codegen_retag_options",
];

// Values for options whose parser takes a string or whose description lists no values, picked
// by hand (`rustc --print code-models` etc. for the enumerations).
const SAMPLES: &[(&str, &[&str])] = &[
    ("-Copt-level", &["0", "1", "2", "3", "s", "z"]),
    ("-Ccode-model", &["tiny", "small", "kernel", "medium", "large"]),
    ("-Crelocation-model", &["static", "pic", "pie", "dynamic-no-pic", "ropi", "rwpi", "ropi-rwpi", "default"]),
    ("-Ztls-model", &["global-dynamic", "local-dynamic", "initial-exec", "local-exec", "emulated"]),
    ("-Ctarget-cpu", &["generic", "native", "x86-64-v2", "x86-64-v3", "x86-64-v4"]),
    ("-Ctarget-feature", &["+avx2", "+avx512f", "-sse4.2", "+crt-static"]),
    ("-Ztune-cpu", &["generic", "znver4"]),
    ("-Zthreads", &["1", "4"]),
    ("-Zlocation-detail", &["none", "file", "line,column"]),
    ("-Zmin-function-alignment", &["16", "64"]),
    ("-Zpatchable-function-entry", &["4", "4,2"]),
    ("-Zmir-enable-passes", &["+Inline", "-GVN", "+DeadStoreElimination-final"]),
    ("-Zremap-cwd-prefix", &["/remapped"]),
    ("-Zsimulate-remapped-rust-src-base", &["/rustc/simulated"]),
    ("-Zhint-msrv", &["1.60.0"]),
    ("-Cmetadata", &["mirth"]),
    ("-Zinstrument-xray", &["always", "never"]),
    ("-Cllvm-args", &["-unroll-threshold=0", "-enable-machine-outliner"]),
    ("-Zcrate-attr", &["allow(unused)"]),
];

static DESC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"pub\(crate\) const (parse_\w+): &str =\s*((?:"(?:[^"\\]|\\.)*"\s*)+|parse_\w+|[^;]+);"#).unwrap());
static PARSER_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^parse_\w+$").unwrap());
static BACKTICKED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());
static OPTION_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\w+): .*?, (parse_\w+), \[(\w+)").unwrap());

fn parse_options(src: &str) -> anyhow::Result<Vec<Opt>> {
    // The parsers' descriptions, joined across lines.
    let mut descs: HashMap<String, String> = DESC.captures_iter(src).map(|c| (c[1].to_owned(), c[2].to_owned())).collect();
    let aliases: Vec<(String, String)> =
        descs.iter().filter(|(_, v)| PARSER_NAME.is_match(v.trim())).map(|(k, v)| (k.clone(), v.trim().to_owned())).collect();
    for (k, v) in aliases {
        let target = descs.get(&v).cloned().unwrap_or_default();
        descs.insert(k, target);
    }
    let enum_values = |parser: &str| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in BACKTICKED.captures_iter(descs.get(parser).map_or("", String::as_str)) {
            let v = &c[1];
            if out.iter().any(|x| x == v) || v.contains(' ') || v.contains('<') || v.contains('=') {
                continue;
            }
            out.push(v.to_owned());
        }
        out
    };
    let mut options = Vec::new();
    for (flag, grp) in [("-C", "CodegenOptions"), ("-Z", "UnstableOptions")] {
        let i = src.find(&format!("options! {{\n    {grp}")).ok_or_else(|| anyhow::anyhow!("no {grp} in options.rs"))?;
        let j = i + src[i..].find("\n}").ok_or_else(|| anyhow::anyhow!("unterminated {grp}"))?;
        for line in src[i..j].lines() {
            let Some(m) = OPTION_LINE.captures(line) else { continue };
            let (name, parser, tracking) = (m[1].replace('_', "-"), m[2].to_owned(), m[3].to_owned());
            let mut values: Vec<Option<String>> = if BOOL.contains(&parser.as_str()) {
                vec![Some("yes".into()), Some("no".into())]
            } else if NO_VALUE.contains(&parser.as_str()) {
                vec![None]
            } else if NUMBER.contains(&parser.as_str()) {
                vec![Some("1".into()), Some("16".into())]
            } else if FREE.contains(&parser.as_str()) {
                vec![]
            } else {
                enum_values(&parser).into_iter().map(Some).collect()
            };
            let full = format!("{flag}{name}");
            if let Some((_, s)) = SAMPLES.iter().find(|(k, _)| *k == full) {
                values = s.iter().map(|v| Some(v.to_string())).collect();
            }
            let free = values.is_empty();
            options.push(Opt { flag: flag.into(), name, parser, tracking, values, free });
        }
    }
    Ok(options)
}

fn arg(o: &Opt, v: &Option<String>) -> String {
    match v {
        None => o.full(),
        Some(v) => format!("{}={v}", o.full()),
    }
}

struct Tried {
    ok: bool,
    warn: bool,
    msg: String,
}

fn try_args(rustc: &Path, work: &Path, argv: &[&str]) -> Tried {
    let d = tempfile::tempdir_in(work).expect("scratch");
    let lib = d.path().join("lib.rs");
    let _ = std::fs::write(&lib, "pub fn f(x: u32) -> u32 { x.wrapping_mul(3) }\n");
    let mut cmd = Command::new(rustc);
    cmd.args(["--edition", "2021", "--crate-type", "lib", "--emit=metadata", "-o"])
        .arg(d.path().join("out.rmeta"))
        .args(argv)
        .arg(&lib)
        .current_dir(d.path());
    match run_command(cmd, Duration::from_secs(60)) {
        Ok(f) if f.exit == Exit::Timeout => Tried { ok: false, warn: false, msg: "timeout".into() },
        Ok(f) => {
            let err = f.stderr_text();
            let first = err.lines().find(|l| l.starts_with("error") || l.starts_with("warning")).unwrap_or("");
            Tried { ok: f.success(), warn: err.contains("warning"), msg: first.chars().take(200).collect() }
        }
        Err(e) => Tried { ok: false, warn: false, msg: e.to_string() },
    }
}

#[derive(Serialize, Deserialize)]
struct Pair {
    a: String,
    b: String,
    ok: bool,
    warn: bool,
    msg: String,
}

/// A pairwise covering array, built greedily: each row is chosen among random candidates to
/// cover the most uncovered pairs. Values are indices into each domain; index 0 is the
/// option's absence.
fn covering_array(domains: &[Vec<Option<String>>], forbidden: &HashSet<(Option<String>, Option<String>)>, seed: u64) -> Vec<Vec<usize>> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let n = domains.len();
    let bad = |i: usize, a: usize, j: usize, b: usize| forbidden.contains(&(domains[i][a].clone(), domains[j][b].clone()));
    let mut uncovered: HashSet<(usize, usize, usize, usize)> = HashSet::new();
    for i in 0..n {
        for j in i + 1..n {
            for a in 0..domains[i].len() {
                for b in 0..domains[j].len() {
                    if !bad(i, a, j, b) {
                        uncovered.insert((i, a, j, b));
                    }
                }
            }
        }
    }
    let mut rows = Vec::new();
    while let Some(&target) = uncovered.iter().min() {
        let (mut best, mut best_gain) = (Vec::new(), -1i64);
        for _ in 0..30 {
            let mut row: Vec<usize> = domains.iter().map(|d| rng.random_range(..d.len())).collect();
            row[target.0] = target.1;
            row[target.2] = target.3;
            // repair forbidden pairs by falling back to absence
            for i in 0..n {
                for j in i + 1..n {
                    if bad(i, row[i], j, row[j]) {
                        if j != target.0 && j != target.2 {
                            row[j] = 0;
                        } else if i != target.0 && i != target.2 {
                            row[i] = 0;
                        }
                    }
                }
            }
            let mut gain = 0i64;
            for i in 0..n {
                for j in i + 1..n {
                    gain += uncovered.contains(&(i, row[i], j, row[j])) as i64;
                }
            }
            if gain > best_gain {
                best = row;
                best_gain = gain;
            }
        }
        for i in 0..n {
            for j in i + 1..n {
                uncovered.remove(&(i, best[i], j, best[j]));
            }
        }
        rows.push(best);
    }
    rows
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let src = std::fs::read_to_string(args.source.join("compiler/rustc_session/src/options.rs"))?;
    let options = parse_options(&src)?;
    std::fs::write(args.work.join("options.json"), to_json_indent1(&options))?;
    let walkable: Vec<&Opt> = options.iter().filter(|o| !o.free).collect();
    println!("{} options: {} with enumerable values, {} free-form", options.len(), walkable.len(), options.len() - walkable.len());
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;

    let singles_path = args.work.join("singles.json");
    let mut singles: Vec<Single> =
        if singles_path.exists() { serde_json::from_str(&std::fs::read_to_string(&singles_path)?)? } else { Vec::new() };
    // Try the values not tried yet (all of them on a first run).
    let done: HashSet<String> = singles.iter().map(|s| s.arg.clone()).collect();
    let jobs: Vec<(&Opt, &Option<String>)> =
        walkable.iter().flat_map(|o| o.values.iter().map(move |v| (*o, v))).filter(|(o, v)| !done.contains(&arg(o, v))).collect();
    if !jobs.is_empty() {
        let results: Vec<Tried> = pool.install(|| jobs.par_iter().map(|(o, v)| try_args(&args.rustc, &args.work, &[&arg(o, v)])).collect());
        for ((o, v), r) in jobs.iter().zip(results) {
            singles.push(Single { arg: arg(o, v), option: o.full(), value: (*v).clone(), ok: r.ok, warn: r.warn, msg: r.msg });
        }
        std::fs::write(&singles_path, to_json_indent1(&singles))?;
    }
    let mut accepted: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in singles.iter().filter(|s| s.ok) {
        accepted.entry(s.option.clone()).or_default().push(s.arg.clone());
    }
    println!(
        "{} single values tried, {} accepted ({} options with at least one)",
        singles.len(),
        accepted.values().map(Vec::len).sum::<usize>(),
        accepted.len()
    );

    let mut rejected_pairs: HashSet<(String, String)> = HashSet::new();
    if !args.skip_pairs {
        let pairs_path = args.work.join("pairs.json");
        let pairs: Vec<Pair> = if pairs_path.exists() {
            serde_json::from_str(&std::fs::read_to_string(&pairs_path)?)?
        } else {
            let opts: Vec<&String> = accepted.keys().collect();
            let mut jobs: Vec<(String, String)> = Vec::new();
            for (i, x) in opts.iter().enumerate() {
                for y in &opts[i + 1..] {
                    for a in &accepted[*x] {
                        for b in &accepted[*y] {
                            jobs.push((a.clone(), b.clone()));
                        }
                    }
                }
            }
            // A value rejected alone may need another option: try it with every accepted
            // value of every other option.
            for r in singles.iter().filter(|s| !s.ok) {
                for y in opts.iter().filter(|y| ***y != r.option) {
                    for b in &accepted[*y] {
                        jobs.push((r.arg.clone(), b.clone()));
                    }
                }
            }
            println!("{} pairs to try", jobs.len());
            let results: Vec<Tried> = pool.install(|| jobs.par_iter().map(|(a, b)| try_args(&args.rustc, &args.work, &[a, b])).collect());
            let pairs: Vec<Pair> =
                jobs.into_iter().zip(results).map(|((a, b), r)| Pair { a, b, ok: r.ok, warn: r.warn, msg: r.msg }).collect();
            std::fs::write(&pairs_path, to_json_indent1(&pairs))?;
            pairs
        };
        let alone: HashSet<&String> = singles.iter().filter(|s| s.ok).map(|s| &s.arg).collect();
        rejected_pairs =
            pairs.iter().filter(|x| !x.ok && alone.contains(&x.a) && alone.contains(&x.b)).map(|x| (x.a.clone(), x.b.clone())).collect();
        // Insertion-ordered, as the Python dict was.
        let mut requires: Vec<(String, Vec<String>)> = Vec::new();
        for x in pairs.iter().filter(|x| x.ok && !alone.contains(&x.a)) {
            match requires.iter_mut().find(|(k, _)| *k == x.a) {
                Some((_, v)) => v.push(x.b.clone()),
                None => requires.push((x.a.clone(), vec![x.b.clone()])),
            }
        }
        println!(
            "{} pairs tried; {} pairs of values accepted alone are rejected together; {} values rejected alone are accepted with another option",
            pairs.len(),
            rejected_pairs.len(),
            requires.len()
        );
        std::fs::write(args.work.join("requires.json"), to_json_indent1(&OrderedMap(&requires)))?;
    }

    let forbidden: HashSet<(Option<String>, Option<String>)> = rejected_pairs
        .iter()
        .flat_map(|(a, b)| [(Some(a.clone()), Some(b.clone())), (Some(b.clone()), Some(a.clone()))])
        .collect();
    let byopt: HashMap<String, &Opt> = options.iter().map(|o| (o.full(), o)).collect();
    let mut results: Vec<(String, usize)> = Vec::new();
    for (label, pred) in [
        ("untracked", (|o: &Opt| o.tracking == "UNTRACKED") as fn(&Opt) -> bool),
        ("tracked", |o: &Opt| o.tracking != "UNTRACKED"),
        ("all", |_: &Opt| true),
    ] {
        let opts: Vec<&String> = accepted.keys().filter(|k| byopt.get(*k).is_some_and(|o| pred(o))).collect();
        if opts.len() <= 1 {
            continue;
        }
        let domains: Vec<Vec<Option<String>>> =
            opts.iter().map(|o| std::iter::once(None).chain(accepted[*o].iter().cloned().map(Some)).collect()).collect();
        let mut sizes: Vec<usize> = domains.iter().map(Vec::len).collect();
        sizes.sort_by(|a, b| b.cmp(a));
        let total: f64 = sizes.iter().map(|&s| s as f64).product();
        let lower = if sizes.len() > 1 { sizes[0] * sizes[1] } else { sizes[0] };
        let rows = covering_array(&domains, &forbidden, 0);
        println!(
            "{label}: {} options, all combinations {}, pairwise covering array {} rows (lower bound {lower})",
            opts.len(),
            sci(total),
            rows.len()
        );
        results.push((label.to_owned(), rows.len()));
    }
    let covering: Vec<String> = results.iter().map(|(k, v)| format!("\"{k}\": {v}")).collect();
    std::fs::write(args.work.join("covering.json"), format!("{{{}}}", covering.join(", ")))?;
    Ok(ExitCode::SUCCESS)
}

/// Python's `{:.3e}`.
fn sci(x: f64) -> String {
    let s = format!("{x:.3e}");
    match s.split_once('e') {
        Some((m, e)) => {
            let e: i32 = e.parse().unwrap_or(0);
            format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
        }
        None => s,
    }
}

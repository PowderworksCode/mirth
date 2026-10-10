//! Scaling and budgets: compile time, memory, future sizes and stack frames must grow at most
//! about linearly with the size of a program of a fixed shape.
//!
//! Each generator writes a program of size N for N in a doubling series. For each N: the
//! compiler's user CPU time and peak memory (wait4's rusage), the largest `sub $X, %rsp` in the
//! assembly, and for some shapes a size the program reports (`size_of_val` of a future). The
//! growth exponent k (value ~ N^k) is the log-log slope over the three largest sizes. Findings:
//! k above --max-exponent for time or memory, above 1.3 for a frame or a size, or a timeout.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    #[arg(long)]
    work: PathBuf,
    /// Comma-separated shapes (default: all).
    #[arg(long)]
    only: Option<String>,
    #[arg(long, default_value = "0,2")]
    opt: String,
    #[arg(long, default_value = "50,100,200,400,800")]
    sizes: String,
    #[arg(long, default_value_t = 300)]
    timeout: u64,
    #[arg(long, default_value_t = 1.6)]
    max_exponent: f64,
    #[arg(long, default_value_t = 4)]
    jobs: usize,
}

fn g_fields(n: usize) -> String {
    let mut f = String::new();
    for i in 0..n {
        let _ = writeln!(f, "    pub f{i}: u{},", 8 << (i % 4));
    }
    format!("#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]\npub struct S {{\n{f}}}\nfn main() {{ let s = S::default(); println!(\"{{}}\", format!(\"{{:?}}\", s.clone()).len()); }}\n")
}

fn g_enum(n: usize) -> String {
    let variants: String = (0..n).map(|i| format!("    V{i}(u32),\n")).collect();
    let arms: String = (0..n).map(|i| format!("        E::V{i}(x) => x + {i},\n")).collect();
    format!("#[derive(Debug, Clone, PartialEq)]\npub enum E {{\n{variants}}}\npub fn f(e: &E) -> u32 {{\n    match *e {{\n{arms}    }}\n}}\nfn main() {{ println!(\"{{}}\", f(&E::V0(1))); }}\n")
}

fn g_nested_generic(n: usize) -> String {
    let ty = (0..n).fold("u8".to_owned(), |t, _| format!("W<{t}>"));
    format!("#[derive(Clone, Debug, Default)] pub struct W<T>(T);\npub trait T {{ fn t(&self) -> usize; }}\nimpl T for u8 {{ fn t(&self) -> usize {{ 1 }} }}\nimpl<X: T> T for W<X> {{ fn t(&self) -> usize {{ self.0.t() + 1 }} }}\nfn main() {{ let v: {ty} = Default::default(); println!(\"{{}}\", v.t()); }}\n")
}

fn g_iter_chain(n: usize) -> String {
    let chain: String = (0..n).map(|i| format!(".map(|x| x.wrapping_add({i}))")).collect();
    format!("fn main() {{ let s: u64 = (0u64..10){chain}.sum(); println!(\"{{}}\", s); }}\n")
}

fn g_async_forward(n: usize) -> String {
    let mut fns = vec!["async fn f0(x: [u8; 64]) -> u8 { x[0] }".to_owned()];
    fns.extend((1..n).map(|i| format!("async fn f{i}(x: [u8; 64]) -> u8 {{ f{}(x).await }}", i - 1)));
    format!("{}\nfn main() {{ let fut = f{}([1; 64]); println!(\"SIZE {{}}\", std::mem::size_of_val(&fut)); }}\n", fns.join("\n"), n - 1)
}

fn g_seq_calls(n: usize) -> String {
    let calls: String = (0..n).map(|i| format!("    let a{i} = big({i}); acc ^= a{i}[{}];\n", i % 512)).collect();
    format!("#[inline(never)] fn big(x: u64) -> [u64; 512] {{ [x; 512] }}\n#[inline(never)] pub fn many() -> u64 {{\n    let mut acc = 0u64;\n{calls}    acc\n}}\nfn main() {{ println!(\"{{}}\", many()); }}\n")
}

fn g_trait_impls(n: usize) -> String {
    let impls: String = (0..n).map(|i| format!("pub struct S{i}; impl Tr for S{i} {{ fn v(&self) -> u32 {{ {i} }} }}\n")).collect();
    let uses = (0..n).map(|i| format!("S{i}.v()")).collect::<Vec<_>>().join(" + ");
    format!("pub trait Tr {{ fn v(&self) -> u32; }}\n{impls}fn main() {{ println!(\"{{}}\", {uses}); }}\n")
}

fn g_nested_expr(n: usize) -> String {
    let expr = (0..n).fold("1u64".to_owned(), |e, i| format!("({e} + {})", i % 7));
    format!("fn main() {{ let x = std::hint::black_box({expr}); println!(\"{{}}\", x); }}\n")
}

#[derive(Clone, Copy, PartialEq)]
enum Extra {
    None,
    RunSize,
    Frame,
}

/// (name, generator, what else to measure, largest meaningful N)
const SHAPES: &[(&str, fn(usize) -> String, Extra, usize)] = &[
    ("fields", g_fields, Extra::None, usize::MAX),
    ("enum", g_enum, Extra::None, usize::MAX),
    ("nested-generic", g_nested_generic, Extra::None, 120),
    ("iter-chain", g_iter_chain, Extra::None, usize::MAX),
    ("async-forward", g_async_forward, Extra::RunSize, 200),
    ("seq-calls", g_seq_calls, Extra::Frame, usize::MAX),
    ("trait-impls", g_trait_impls, Extra::None, usize::MAX),
    ("nested-expr", g_nested_expr, Extra::None, 400),
];

#[derive(Serialize, Default, Clone)]
struct Row {
    n: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rss_kb: Option<i64>,
    max_frame: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_size: Option<u64>,
    timeout: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    error: String,
}

/// Compile in `dir` with rusage from wait4; None user time on timeout.
fn compile_measured(args: &Args, dir: &Path, opt: u32) -> Row {
    let mut child = match Command::new(&args.rustc)
        .args(["m.rs", "--edition", "2021", &format!("-Copt-level={opt}"), "-o", "m", "--emit=link,asm"])
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return Row { error: e.to_string(), ..Default::default() },
    };
    let pid = child.id() as libc::pid_t;
    let deadline = Instant::now() + Duration::from_secs(args.timeout);
    let mut status: libc::c_int = 0;
    // SAFETY: rusage is plain data; wait4 fills it for the child we spawned.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    loop {
        // SAFETY: waiting on our own child; status and usage outlive the call.
        let r = unsafe { libc::wait4(pid, &mut status, libc::WNOHANG, &mut usage) };
        if r == pid {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            // SAFETY: reap the killed child.
            unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
            return Row { timeout: true, ..Default::default() };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let _ = e.read_to_string(&mut err);
    }
    if !(libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0) {
        return Row { error: err.chars().rev().take(500).collect::<String>().chars().rev().collect(), ..Default::default() };
    }
    let user = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1e6;
    Row { user: Some(user), rss_kb: Some(usage.ru_maxrss), ..Default::default() }
}

static SUB_RSP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"subq?\s+\$(0x[0-9a-f]+|\d+),\s*%rsp").unwrap());
static SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SIZE (\d+)").unwrap());

fn measure(args: &Args, source: &str, opt: u32) -> Row {
    let dir = tempfile::tempdir_in(&args.work).expect("scratch");
    let _ = std::fs::write(dir.path().join("m.rs"), source);
    let mut row = compile_measured(args, dir.path(), opt);
    if row.user.is_none() {
        return row;
    }
    let asm = std::fs::read_to_string(dir.path().join("m.s")).unwrap_or_default();
    row.max_frame = SUB_RSP
        .captures_iter(&asm)
        .filter_map(|c| {
            let v = &c[1];
            if let Some(h) = v.strip_prefix("0x") { u64::from_str_radix(h, 16).ok() } else { v.parse().ok() }
        })
        .max()
        .unwrap_or(0);
    if let Ok(out) = Command::new(dir.path().join("m")).output() {
        row.run_size = SIZE.captures(&String::from_utf8_lossy(&out.stdout)).and_then(|c| c[1].parse().ok());
    }
    row
}

/// The log-log slope over the three largest positive points.
fn exponent(points: &[(usize, f64)]) -> Option<f64> {
    let pts: Vec<(f64, f64)> = points.iter().filter(|(_, v)| *v > 0.0).map(|&(n, v)| ((n as f64).ln(), v.ln())).collect();
    if pts.len() < 3 {
        return None;
    }
    let pts = &pts[pts.len() - 3..];
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / 3.0;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / 3.0;
    let den: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
    (den > 0.0).then(|| pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / den)
}

#[derive(Serialize)]
struct Res {
    shape: String,
    opt: u32,
    rows: Vec<Row>,
    k_time: Option<f64>,
    k_rss: Option<f64>,
    k_frame: Option<f64>,
    k_size: Option<f64>,
    found: Vec<String>,
}

fn one(args: &Args, sizes: &[usize], shape: &(&str, fn(usize) -> String, Extra, usize), opt: u32) -> Res {
    let (name, gen_fn, extra, cap) = *shape;
    let mut rows = Vec::new();
    // A shape capped below the series gets the series scaled to end at its cap, so it still has
    // enough points for a fit.
    let scaled: Vec<usize>;
    let sizes = if sizes.iter().filter(|&&n| n <= cap).count() >= 3 {
        sizes
    } else {
        let top = *sizes.last().unwrap_or(&1) as f64;
        scaled = sizes.iter().map(|&n| ((n as f64 / top) * cap as f64).round().max(1.0) as usize).collect();
        &scaled
    };
    for &n in sizes.iter().filter(|&&n| n <= cap) {
        let mut r = measure(args, &gen_fn(n), opt);
        r.n = n;
        let stop = r.timeout || !r.error.is_empty();
        rows.push(r);
        if stop {
            break;
        }
    }
    let ok: Vec<&Row> = rows.iter().filter(|r| r.user.is_some()).collect();
    let first_user = ok.first().and_then(|r| r.user).unwrap_or(0.0);
    // Below half a second the timer's noise decides the slope.
    let k_time = if ok.last().and_then(|r| r.user).unwrap_or(0.0) < 0.5 {
        None
    } else {
        exponent(&ok.iter().map(|r| (r.n, (r.user.unwrap() - first_user * 0.5).max(1e-3))).collect::<Vec<_>>())
    };
    let k_rss = exponent(&ok.iter().map(|r| (r.n, r.rss_kb.unwrap_or(0) as f64)).collect::<Vec<_>>());
    let k_frame = (extra == Extra::Frame).then(|| exponent(&ok.iter().map(|r| (r.n, r.max_frame as f64)).collect::<Vec<_>>())).flatten();
    let k_size = (extra == Extra::RunSize).then(|| exponent(&ok.iter().map(|r| (r.n, r.run_size.unwrap_or(0) as f64)).collect::<Vec<_>>())).flatten();
    let mut found = Vec::new();
    if let Some(last) = rows.last() {
        if last.timeout {
            found.push(format!("timeout at N={}", last.n));
        }
        if !last.error.is_empty() {
            found.push(format!("error at N={}: {}", last.n, last.error.chars().rev().take(200).collect::<String>().chars().rev().collect::<String>()));
        }
    }
    for (label, k, limit) in [("k_time", k_time, args.max_exponent), ("k_rss", k_rss, args.max_exponent), ("k_frame", k_frame, 1.3), ("k_size", k_size, 1.3)] {
        if let Some(k) = k
            && k > limit
        {
            found.push(format!("{label} = {k:.2}"));
        }
    }
    Res { shape: name.into(), opt, rows, k_time, k_rss, k_frame, k_size, found }
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let sizes: Vec<usize> = args.sizes.split(',').filter_map(|s| s.parse().ok()).collect();
    let opts: Vec<u32> = args.opt.split(',').filter_map(|s| s.parse().ok()).collect();
    let wanted: Option<Vec<&str>> = args.only.as_deref().map(|o| o.split(',').collect());
    let jobs: Vec<(&(&str, fn(usize) -> String, Extra, usize), u32)> = SHAPES
        .iter()
        .filter(|s| wanted.as_ref().is_none_or(|w| w.contains(&s.0)))
        .flat_map(|s| opts.iter().map(move |&o| (s, o)))
        .collect();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;
    let results: Vec<Res> = pool.install(|| jobs.par_iter().map(|(s, o)| one(&args, &sizes, s, *o)).collect());
    std::fs::write(args.work.join("results.json"), serde_json::to_string_pretty(&results)?)?;
    for r in &results {
        let last = r.rows.iter().rev().find(|x| x.user.is_some());
        let ks: Vec<String> = [("k_time", r.k_time), ("k_rss", r.k_rss), ("k_frame", r.k_frame), ("k_size", r.k_size)]
            .iter()
            .filter_map(|(l, k)| k.map(|k| format!("{l}={k:.2}")))
            .collect();
        println!(
            "{:15} O{}  N<={}  user {:.1}s  rss {}MB  {}  {}",
            r.shape,
            r.opt,
            last.map_or(0, |x| x.n),
            last.and_then(|x| x.user).unwrap_or(0.0),
            last.and_then(|x| x.rss_kb).unwrap_or(0) / 1024,
            ks.join(" "),
            r.found.join("; ")
        );
    }
    Ok(ExitCode::SUCCESS)
}

//! Compiler runs that rustc's test suites hardly make, for coverage (`mirth-lab callgraph
//! --gaps` lists what is left): run them with the coverage-instrumented compiler and MIRTH_OUT
//! set, and fold the logs with `mirth-lab coverage-compact`.
//!
//! - prints: every `--print` request, on the host and on every target;
//! - targets: `tests/auxiliary/minicore.rs` and a file of functions with every kind of argument
//!   and return value, compiled to an object for every target (each target's ABI, layout and
//!   codegen code);
//! - links: for every target, a `no_main` binary, a cdylib, a staticlib and a dylib on minicore,
//!   linked with `-Clinker=true` (the linker command each target's linker flavor builds, without
//!   the linker), with linker options;
//! - dumps: each test of the list (`mirth-lab ui-coverage pick`) with each debugging and
//!   printing option (`-Zunpretty=`, `-Zdump-mir`, `-Zprint-type-sizes`, statistics, profiling, ...).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use mirth_lab::rustc::run_command;
use mirth_lab::uitest;
use rayon::prelude::*;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// The rust checkout (tests/auxiliary/minicore.rs, tests/ui).
    #[arg(long)]
    rust: PathBuf,
    /// The picked tests (picked.json of `mirth-lab ui-coverage pick`).
    #[arg(long)]
    list: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value_t = 8)]
    jobs: usize,
    #[arg(long, default_value = "prints,targets,dumps,links")]
    only: String,
}

const PRINTS: &[&str] = &[
    "all-target-specs-json", "backend-has-mnemonic", "backend-has-zstd", "calling-conventions", "cfg", "check-cfg",
    "code-models", "crate-name", "crate-root-lint-levels", "deployment-target", "file-names", "host-tuple", "link-args",
    "native-static-libs", "relocation-models", "split-debuginfo", "stack-protector-strategies", "supported-crate-types",
    "sysroot", "target-cpus", "target-features", "target-libdir", "target-list", "target-spec-json",
    "target-spec-json-schema", "tls-models", "wasm-proc-macro-tuple",
];
const PER_TARGET: &[&str] = &[
    "cfg", "target-spec-json", "target-cpus", "target-features", "calling-conventions", "code-models",
    "relocation-models", "tls-models", "stack-protector-strategies", "split-debuginfo", "supported-crate-types",
    "deployment-target", "check-cfg",
];

/// Every kind of argument and return value, for each target's calling convention.
const ABI: &str = r#"
#![feature(no_core, lang_items, rustc_attrs, c_variadic, f16, f128)]
#![no_core]
#![crate_type = "lib"]
#![allow(improper_ctypes_definitions, unused)]
extern crate minicore;
use minicore::*;

#[repr(C)] pub struct Small { a: u8, b: u16 }
#[repr(C)] pub struct Pair { a: u64, b: u64 }
#[repr(C)] pub struct Big { a: [u64; 8] }
#[repr(C)] pub struct Floats { a: f32, b: f64 }
#[repr(C)] pub struct Mixed { a: f32, b: u32 }
#[repr(C)] pub union U { a: u32, b: f32 }
#[repr(C)] pub struct Hfa { a: f32, b: f32, c: f32, d: f32 }
#[repr(C)] pub struct Empty {}
#[repr(transparent)] pub struct T(u64);
#[repr(C, packed)] pub struct Packed { a: u8, b: u32 }
#[repr(C, align(16))] pub struct Aligned { a: u8 }

#[no_mangle] pub extern "C" fn c_small(x: Small) -> Small { x }
#[no_mangle] pub extern "C" fn c_pair(x: Pair) -> Pair { x }
#[no_mangle] pub extern "C" fn c_big(x: Big) -> Big { x }
#[no_mangle] pub extern "C" fn c_floats(x: Floats, y: f32, z: f64) -> Floats { x }
#[no_mangle] pub extern "C" fn c_mixed(x: Mixed) -> Mixed { x }
#[no_mangle] pub extern "C" fn c_union(x: U) -> U { x }
#[no_mangle] pub extern "C" fn c_hfa(x: Hfa) -> Hfa { x }
#[no_mangle] pub extern "C" fn c_empty(x: Empty) -> Empty { x }
#[no_mangle] pub extern "C" fn c_transparent(x: T) -> T { x }
#[no_mangle] pub extern "C" fn c_packed(x: Packed) -> Packed { x }
#[no_mangle] pub extern "C" fn c_aligned(x: Aligned) -> Aligned { x }
#[no_mangle] pub extern "C" fn c_ints(a: i8, b: u16, c: i32, d: u64, e: i128, f: u128, g: bool, h: char) -> i128 { e }
#[no_mangle] pub extern "C" fn c_ptrs(a: *const u8, b: &u32, c: &mut [u8; 3], f: extern "C" fn()) -> *const u8 { a }
#[no_mangle] pub extern "C" fn c_many(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64, h: u64, i: u64, j: Pair, k: f64, l: f64, m: f64, n: f64, o: f64, p: f64, q: f64, r: f64, s: f64) -> u64 { a }
#[no_mangle] pub unsafe extern "C" fn c_variadic(a: u32, mut args: ...) -> u32 { a }
pub fn rust_all(a: Small, b: Pair, c: Big, d: Floats, e: (u8, u64), f: [u32; 5], g: &[u8], h: &str, i: u128) -> Big { c }
pub fn rust_f16(a: f16, b: f128) -> f128 { b }
#[no_mangle] pub extern "C" fn c_f16(a: f16, b: f128) -> f128 { b }
#[no_mangle] pub extern "system" fn system(a: Pair) -> Pair { a }
#[no_mangle] pub extern "C-unwind" fn c_unwind(a: Pair) -> Pair { a }
pub static TABLE: [extern "C" fn(Pair) -> Pair; 2] = [c_pair, c_unwind_shim];
extern "C" fn c_unwind_shim(a: Pair) -> Pair { a }
extern "C" { fn imported(a: Big, b: Floats) -> Hfa; }
pub unsafe fn call_imported(a: Big, b: Floats) -> Hfa { imported(a, b) }
"#;

const LINKED: &str = r#"
#![feature(no_core, lang_items)]
#![no_core]
#![no_main]
extern crate minicore;
#[no_mangle] pub extern "C" fn exported(a: u32) -> u32 { a }
#[no_mangle] pub static DATA: u32 = 7;
#[link(name = "c")] extern "C" { fn puts(p: *const u8) -> i32; }
#[link(name = "m", kind = "static")] extern "C" {}
#[link(name = "framework_like", kind = "dylib", modifiers = "+verbatim")] extern "C" {}
"#;

const LINK_OPTIONS: &[&[&str]] = &[
    &[], &["-Cprefer-dynamic", "-Crelocation-model=pic"], &["-Cstrip=symbols", "-Clink-dead-code"],
    &["-Clink-self-contained=yes"], &["-Cdebuginfo=2", "-Csplit-debuginfo=packed"],
    &["-Clink-arg=-Wl,--foo", "-Clink-args=-x -y", "-Zpre-link-args=-z"],
    &["-Cdefault-linker-libraries", "-Zlink-native-libraries=no"], &["-Ccontrol-flow-guard"],
    &["-Zstaticlib-allow-rdylib-deps"], &["-Copt-level=s", "-Clto=fat"], &["-Ccode-model=large"],
];

const UNPRETTY: &[&str] = &[
    "normal", "expanded", "expanded,identified", "expanded,hygiene", "ast-tree", "ast-tree,expanded", "hir",
    "hir,identified", "hir,typed", "hir-tree", "thir-tree", "thir-flat", "mir", "stable-mir", "mir-cfg",
];
const DUMPS: &[&[&str]] = &[
    &["-Zdump-mir=all", "-Zdump-mir-dataflow", "-Zdump-mir-graphviz", "-Zmir-include-spans=on"],
    &["-Zprint-type-sizes"], &["-Zprint-mono-items=yes", "--emit=link"], &["-Zmeta-stats"], &["-Zhir-stats"],
    &["-Zinput-stats"], &["-Zself-profile", "-Zself-profile-events=all"], &["-Ztime-passes"],
    &["-Zquery-dep-graph", "-Zdump-dep-graph", "-Cincremental=inc"], &["-Zincremental-info", "-Cincremental=inc"],
    &["-Zdump-mono-stats", "-Zdump-mono-stats-format=json", "--emit=link"], &["-Zprint-codegen-stats", "--emit=link"],
    &["-Zvalidate-mir", "-Zlint-mir", "-Zmir-opt-level=4"], &["-Zverbose-internals", "-Zidentify-regions"],
    &["-Ztrack-diagnostics", "-Zteach"], &["-Zthreads=4"], &["-Zpolonius=next"], &["-Zinline-mir", "-Zmir-opt-level=3"],
    &["-Zrandomize-layout"], &["-Zwrite-long-types-to-disk=no", "-Zverbose-internals"],
    &["-Zunleash-the-miri-inside-of-you"], &["-Zno-analysis"], &["-Zprofile-closures"], &["-Zui-testing"],
    &["-Cinstrument-coverage", "--emit=link"], &["-Zemit-stack-sizes", "--emit=link"],
    &["--error-format=json", "--json=diagnostic-rendered-ansi,artifacts,future-incompat,unused-externs"],
    &["--error-format=human-annotate-rs"], &["--error-format=short"], &["-Zterminal-urls=yes", "--color=always"],
    &["-Wunused", "-Wrust-2018-idioms", "-Wrust-2021-compatibility", "-Wrust-2024-compatibility", "-Wclippy::all"],
    &["-Fwarnings", "--cap-lints=warn"], &["-Zcodegen-source-order", "--emit=link"],
];

/// The debugging and printing options: each `-Zunpretty=` mode, then the rest.
fn dumps_options() -> Vec<Vec<String>> {
    let unpretty = UNPRETTY.iter().map(|m| vec![format!("-Zunpretty={m}")]);
    unpretty.chain(DUMPS.iter().map(|o| o.iter().map(|s| s.to_string()).collect())).collect()
}

/// Run with RUSTC_BOOTSTRAP; true when it exits 0.
fn ok<S: AsRef<std::ffi::OsStr>>(program: &Path, argv: &[S], cwd: &Path, secs: u64) -> bool {
    let mut cmd = Command::new(program);
    cmd.args(argv).current_dir(cwd).env("RUSTC_BOOTSTRAP", "1");
    run_command(cmd, Duration::from_secs(secs)).is_ok_and(|f| f.success())
}

fn targets(args: &Args) -> Vec<String> {
    let out = Command::new(&args.rustc).args(["--print", "target-list"]).env("RUSTC_BOOTSTRAP", "1").output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().map(str::to_owned).collect()).unwrap_or_default()
}

fn scratch(args: &Args) -> tempfile::TempDir {
    tempfile::tempdir_in(&args.work).expect("scratch directory")
}

fn prints(args: &Args) {
    let d = scratch(args);
    let empty = d.path().join("lib.rs");
    let _ = std::fs::write(&empty, "");
    let empty = empty.to_string_lossy().into_owned();
    let mut jobs: Vec<Vec<String>> = PRINTS.iter().map(|k| vec!["--print".into(), k.to_string(), "-Zunstable-options".into(), empty.clone()]).collect();
    for target in targets(args) {
        for k in PER_TARGET {
            jobs.push(vec!["--print".into(), k.to_string(), "--target".into(), target.clone(), "-Zunstable-options".into(), empty.clone()]);
        }
    }
    let succeeded = jobs.par_iter().filter(|j| ok(&args.rustc, j, d.path(), 300)).count();
    println!("prints: {} runs, {succeeded} succeeded", jobs.len());
}

/// The common arguments for a `no_core` build for `target` into `d`.
fn target_base(target: &str, d: &Path) -> Vec<String> {
    ["--target", target, "-Zunstable-options", "--edition", "2021", "-Cpanic=abort", "--out-dir"]
        .iter()
        .map(|s| s.to_string())
        .chain([d.to_string_lossy().into_owned()])
        .collect()
}

fn one_target(args: &Args, target: &str) -> Vec<i32> {
    let d = scratch(args);
    let p = d.path();
    let _ = std::fs::write(p.join("abi.rs"), ABI);
    let minicore = args.rust.join("tests/auxiliary/minicore.rs").to_string_lossy().into_owned();
    let base = target_base(target, p);
    let code = |extra: &[String]| {
        let mut cmd = Command::new(&args.rustc);
        cmd.args(&base).args(extra).current_dir(p).env("RUSTC_BOOTSTRAP", "1");
        match run_command(cmd, Duration::from_secs(300)) {
            Ok(f) => match f.exit {
                mirth_lab::rustc::Exit::Code(c) => c,
                _ => -1,
            },
            Err(_) => -1,
        }
    };
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let mut results = vec![code(&[s(&["--crate-type", "rlib", "--crate-name", "minicore", "-Copt-level=1", "--emit=link,obj"]), vec![minicore]].concat())];
    if results[0] == 0 {
        let rlib = format!("minicore={}/libminicore.rlib", p.display());
        for opt in ["0", "3"] {
            results.push(code(&[s(&["--emit=obj,asm,llvm-ir"]), vec![format!("-Copt-level={opt}")], s(&["-Cdebuginfo=2", "--extern"]), vec![rlib.clone()], s(&["abi.rs"])].concat()));
        }
    }
    results
}

fn cross(args: &Args) -> anyhow::Result<()> {
    let done: Vec<(String, Vec<i32>)> = targets(args).into_par_iter().map(|t| { let r = one_target(args, &t); (t, r) }).collect();
    let built = done.iter().filter(|(_, r)| r.len() == 3 && r.iter().all(|c| *c == 0)).count();
    println!("targets: {} targets, {built} built minicore and the ABI file", done.len());
    let map: BTreeMap<String, Vec<i32>> = done.into_iter().collect();
    std::fs::write(args.work.join("targets.json"), mirth_lab::coverage::to_json_indent(&map, 0))?;
    Ok(())
}

fn one_link(args: &Args, target: &str) -> usize {
    let d = scratch(args);
    let p = d.path();
    let _ = std::fs::write(p.join("linked.rs"), LINKED);
    let minicore = args.rust.join("tests/auxiliary/minicore.rs").to_string_lossy().into_owned();
    let mut base = target_base(target, p);
    base.push("-Clinker=true".into());
    let with = |extra: &[&str]| -> Vec<String> { base.iter().cloned().chain(extra.iter().map(|s| s.to_string())).collect() };
    let mut minicore_argv = with(&["--crate-type", "rlib", "--crate-name", "minicore", "--emit=link"]);
    minicore_argv.push(minicore);
    if !ok(&args.rustc, &minicore_argv, p, 300) {
        return 0;
    }
    let rlib = format!("minicore={}/libminicore.rlib", p.display());
    let mut n = 0;
    for options in LINK_OPTIONS {
        for kind in ["bin", "cdylib", "staticlib", "dylib"] {
            let mut argv = with(&["--crate-type", kind, "--extern", &rlib, "-Csave-temps"]);
            argv.extend(options.iter().map(|s| s.to_string()));
            argv.push("linked.rs".into());
            n += ok(&args.rustc, &argv, p, 300) as usize;
        }
    }
    n
}

fn links(args: &Args) {
    let done: Vec<usize> = targets(args).par_iter().map(|t| one_link(args, t)).collect();
    println!("links: {} targets, {} links succeeded", done.len(), done.iter().sum::<usize>());
}

fn dump(args: &Args, options: &[Vec<String>], test: &str) -> usize {
    let path = args.rust.join("tests/ui").join(test);
    let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default()).into_owned();
    let (flags, edition, _, _) = uitest::headers(&text);
    let mut n = 0;
    for extra in options {
        let d = scratch(args);
        let mut argv: Vec<String> = vec![path.to_string_lossy().into_owned(), "--edition".into(), edition.clone().unwrap_or_else(|| "2015".into())];
        if !extra.iter().any(|e| e.starts_with("--emit")) {
            argv.push("--emit=metadata".into());
        }
        argv.extend(["--out-dir".into(), d.path().to_string_lossy().into_owned()]);
        argv.extend(["-Zunstable-options", "-Ainternal_features", "-Aincomplete_features"].map(String::from));
        argv.extend(flags.iter().cloned());
        argv.extend(extra.iter().cloned());
        n += ok(&args.rustc, &argv, d.path(), 120) as usize;
    }
    n
}

fn dumps(args: &Args) -> anyhow::Result<()> {
    let picked: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(&args.list)?)?;
    let tests: Vec<String> = picked
        .iter()
        .filter_map(|t| t.get("test").unwrap_or(t).as_str().map(str::to_owned))
        .collect();
    let options = dumps_options();
    let succeeded: usize = tests.par_iter().map(|t| dump(args, &options, t)).sum();
    println!("dumps: {} tests x {} options, {succeeded} runs succeeded", tests.len(), options.len());
    Ok(())
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;
    let only: Vec<&str> = args.only.split(',').collect();
    pool.install(|| -> anyhow::Result<()> {
        if only.contains(&"prints") {
            prints(&args);
        }
        if only.contains(&"targets") {
            cross(&args)?;
        }
        if only.contains(&"dumps") {
            dumps(&args)?;
        }
        if only.contains(&"links") {
            links(&args);
        }
        Ok(())
    })?;
    Ok(ExitCode::SUCCESS)
}

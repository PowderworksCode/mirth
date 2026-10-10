//! Codegen and target coverage (docs/coverage-plan.md, rank 10), from outputs and existing block
//! coverage, without a rebuild.
//!
//! - LLVM intrinsics: every `llvm.*` name rustc_codegen_llvm can emit (its string literals) against
//!   those in the IR of the runnable UI tests (`--collect`: each test at -Copt-level=0 and 3, with
//!   the calling conventions, linkage kinds and target-feature sets of the IR kept too).
//! - Rust intrinsics: the `sym::<intrinsic>` arms of the codegen intrinsic matches
//!   (rustc_codegen_llvm/src/intrinsic.rs, rustc_codegen_ssa/src/mir/intrinsic.rs), taken or not
//!   by the measured suites (block sites inside the arm).
//! - Calling-convention lowering per architecture: block coverage of each
//!   `rustc_target/src/callconv/<arch>.rs`, and the conventions abi-diff's IR shows per target.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{Compile, Status};
use mirth_lab::sitemap::{self, home, Inputs, SiteMap};
use mirth_lab::uitest::{self, Test};
use quote::ToTokens;
use regex::Regex;
use serde::{Deserialize, Serialize};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long, default_value_os_t = home("mirth-work/campaign/rustc/bin/rustc"))]
    rustc: PathBuf,
    /// Compile the runnable UI tests to LLVM IR and keep what they use (<work>/results.jsonl).
    #[arg(long)]
    collect: bool,
    #[command(flatten)]
    sweep: Sweep,
    #[command(flatten)]
    inputs: Inputs,
    /// An abi-diff work directory (per-target r.ll), for conventions per target.
    #[arg(long)]
    abi: Option<PathBuf>,
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Default)]
struct Rec {
    test: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    #[serde(default)]
    intrinsics: BTreeSet<String>,
    #[serde(default)]
    cc: BTreeSet<String>,
    #[serde(default)]
    linkage: BTreeSet<String>,
    #[serde(default)]
    features: BTreeSet<String>,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        Vec::new()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

static LLVM_CALL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"@(llvm\.[\w.]+)").unwrap());
static CC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:define|declare)\b[^@]*?\b(\w+cc|cc \d+)\b").unwrap());
static LINKAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^define\s+(private|internal|available_externally|linkonce_odr|linkonce|weak_odr|weak|common|appending|extern_weak|external)?").unwrap()
});
static FEATURES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""target-features"="([^"]*)""#).unwrap());
/// An intrinsic's overloaded type suffix (`llvm.memcpy.p0.p0.i64` -> `llvm.memcpy`).
static OVERLOAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\.(p\d+|i\d+|f\d+|v\d+[if]\d+|nxv\d+[if]\d+|[a-z]\d+))+$").unwrap());

fn base(name: &str) -> String {
    OVERLOAD.replace(name, "").into_owned()
}

fn scan_ir(ir: &str, rec: &mut Rec) {
    for line in ir.lines() {
        if line.contains("@llvm.") && (line.contains("call ") || line.starts_with("declare")) {
            for c in LLVM_CALL.captures_iter(line) {
                rec.intrinsics.insert(base(&c[1]));
            }
        }
        if line.starts_with("define") || line.starts_with("declare") {
            if let Some(c) = CC.captures(line) {
                rec.cc.insert(c[1].to_owned());
            }
        }
        if line.starts_with("define") {
            rec.linkage.insert(LINKAGE.captures(line).and_then(|c| c.get(1)).map_or("external", |m| m.as_str()).to_owned());
        }
        if let Some(c) = FEATURES.captures(line) {
            rec.features.extend(c[1].split(',').filter(|f| !f.is_empty()).map(str::to_owned));
        }
    }
}

fn collect_one(args: &Args, test: &Test) -> Rec {
    let mut rec = Rec { test: test.rel.clone(), ..Default::default() };
    for opt in ["0", "3"] {
        let dir = driver::scratch_dir(&args.sweep);
        let c = Compile::new(&args.rustc, &test.path, dir.path(), &test.flags, test.edition())
            .emit("llvm-ir")
            .extra([format!("-Copt-level={opt}")])
            .timeout(120)
            .run();
        if !matches!(c.status, Status::Ok) {
            rec.skip = Some(format!("{:?} at -Copt-level={opt}", c.status).to_lowercase());
            continue;
        }
        scan_ir(&std::fs::read_to_string(dir.path().join("prog")).unwrap_or_default(), &mut rec);
    }
    rec
}

/// `llvm.*` names in rustc_codegen_llvm's string literals (templates with `{}` left out).
fn llvm_names(rust: &Path) -> BTreeSet<String> {
    static LIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""(llvm\.[a-z0-9_.]+)""#).unwrap());
    let mut out = BTreeSet::new();
    for (key, path) in sitemap::compiler_files(rust) {
        if key.starts_with("compiler/rustc_codegen_llvm/") {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            out.extend(LIT.captures_iter(&text).map(|c| base(c[1].trim_end_matches('.'))));
        }
    }
    out
}

/// The `sym::name` arms of the intrinsic matches: (file, name, body span).
struct Arms<'a> {
    file: &'a str,
    out: Vec<(String, String, (sitemap::Pos, sitemap::Pos))>,
}

impl<'ast> Visit<'ast> for Arms<'_> {
    fn visit_arm(&mut self, a: &'ast syn::Arm) {
        static SYM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"sym\s*::\s*(\w+)").unwrap());
        let pat = a.pat.to_token_stream().to_string();
        let span = a.body.span();
        let r = ((span.start().line as u32, span.start().column as u32 + 1), (span.end().line as u32, span.end().column as u32 + 1));
        for c in SYM.captures_iter(&pat) {
            self.out.push((self.file.to_owned(), c[1].to_owned(), r));
        }
        visit::visit_arm(self, a);
    }
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    if args.collect {
        let tests = args.sweep.select(uitest::tests(&args.sweep.tests, uitest::RUNNABLE, |_| false));
        println!("{} tests", tests.len());
        return Ok(driver::drive(&tests, &args.sweep, |t| collect_one(&args, t)));
    }
    let inp = &args.inputs;
    let mut out = String::new();
    let mut gaps = String::new();

    // LLVM intrinsics in the IR.
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    let (mut cc, mut linkage, mut features) = (BTreeMap::<String, usize>::new(), BTreeMap::<String, usize>::new(), BTreeMap::<String, usize>::new());
    let mut recs = 0;
    for line in std::fs::read_to_string(args.sweep.work.join("results.jsonl")).unwrap_or_default().lines() {
        let Ok(r) = serde_json::from_str::<Rec>(line) else { continue };
        recs += 1;
        for i in r.intrinsics {
            *used.entry(i).or_default() += 1;
        }
        for (set, map) in [(r.cc, &mut cc), (r.linkage, &mut linkage), (r.features, &mut features)] {
            for x in set {
                *map.entry(x).or_default() += 1;
            }
        }
    }
    // Metadata names and special globals are not calls: kept apart from the intrinsics.
    let (special, names): (BTreeSet<String>, BTreeSet<String>) = llvm_names(&inp.rust)
        .into_iter()
        .partition(|n| ["llvm.loop.", "llvm.ident", "llvm.metadata", "llvm.used", "llvm.compiler.used"].iter().any(|p| n.starts_with(p)));
    let hit: Vec<&String> = names.iter().filter(|n| used.contains_key(*n)).collect();
    let _ = writeln!(out, "LLVM intrinsics: {} named in rustc_codegen_llvm (plus {} metadata names and special globals, not counted); {} appear in the IR of {recs} runnable tests at -O0/-O3 ({:.1}%); {} distinct intrinsics in that IR overall (LLVM adds its own)", names.len(), special.len(), hit.len(), 100.0 * hit.len() as f64 / names.len().max(1) as f64, used.len());
    let _ = writeln!(out, "  calling conventions in the IR: {cc:?}");
    let _ = writeln!(out, "  linkage kinds of definitions: {linkage:?}");
    let _ = writeln!(out, "  target features enabled anywhere: {}", features.len());
    let _ = writeln!(gaps, "# LLVM intrinsics rustc can emit that no runnable test's IR contains: {}\n", names.len() - hit.len());
    for n in names.iter().filter(|n| !used.contains_key(*n)) {
        let _ = writeln!(gaps, "- {n}");
    }

    // Rust intrinsic arms.
    let map = SiteMap::load(&inp.sites);
    let reached = sitemap::reached(inp);
    let mut arms = Vec::new();
    for f in ["compiler/rustc_codegen_llvm/src/intrinsic.rs", "compiler/rustc_codegen_ssa/src/mir/intrinsic.rs"] {
        let text = std::fs::read_to_string(inp.rust.join(f)).unwrap_or_default();
        if map.alignment(f, &text) < 0.9 {
            let _ = writeln!(out, "  {f} moved since the coverage build: skipped");
            continue;
        }
        if let Ok(file) = syn::parse_file(&text) {
            let mut v = Arms { file: f, out: Vec::new() };
            v.visit_file(&file);
            arms.extend(v.out);
        }
    }
    let mut per: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for (file, name, (a, b)) in &arms {
        let inside = map.probe(file, *a, *b, (u32::MAX, 0));
        let e = per.entry(name.clone()).or_default();
        e.0 |= !inside.is_empty();
        e.1 |= inside.iter().any(|s| reached.contains(&s.id));
    }
    let measurable = per.values().filter(|v| v.0).count();
    let taken = per.values().filter(|v| v.1).count();
    let _ = writeln!(out, "Rust intrinsics with their own codegen arm: {}; measurable {measurable}; lowered by a measured run {taken} ({:.1}%)", per.len(), 100.0 * taken as f64 / measurable.max(1) as f64);
    let _ = writeln!(gaps, "\n# Rust intrinsics whose codegen arm never ran: {}\n", measurable - taken);
    for (n, _) in per.iter().filter(|(_, v)| v.0 && !v.1) {
        let _ = writeln!(gaps, "- {n}");
    }

    // Calling-convention lowering per architecture.
    let _ = writeln!(out, "calling-convention lowering per architecture (block sites in rustc_target/src/callconv/<arch>.rs, reached by any suite):");
    let mut rows: Vec<(String, usize, usize)> = map
        .files()
        .filter(|f| f.starts_with("compiler/rustc_target/src/callconv/"))
        .map(|f| {
            let sites = map.probe(f, (0, 0), (u32::MAX, 0), (0, 0));
            (f.trim_start_matches("compiler/rustc_target/src/callconv/").to_owned(), sites.len(), sites.iter().filter(|s| reached.contains(&s.id)).count())
        })
        .collect();
    rows.sort_by(|a, b| (a.2 as f64 / a.1.max(1) as f64).partial_cmp(&(b.2 as f64 / b.1.max(1) as f64)).unwrap());
    let _ = writeln!(gaps, "\n# Calling-convention lowering by architecture\n\n| file | sites | reached | % |\n|---|---:|---:|---:|");
    let zero = rows.iter().filter(|r| r.2 == 0).count();
    for (f, n, h) in &rows {
        let _ = writeln!(gaps, "| {f} | {n} | {h} | {:.1} |", 100.0 * *h as f64 / (*n).max(1) as f64);
    }
    let total: usize = rows.iter().map(|r| r.1).sum();
    let reached_n: usize = rows.iter().map(|r| r.2).sum();
    let _ = writeln!(out, "  {} files, {total} sites, {reached_n} reached ({:.1}%); {zero} files never reached", rows.len(), 100.0 * reached_n as f64 / total.max(1) as f64);

    // Conventions per target, from abi-diff's IR.
    if let Some(abi) = &args.abi {
        let mut per_target: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(abi) {
            for e in entries.flatten() {
                let ll = e.path().join("r.ll");
                if let Ok(ir) = std::fs::read_to_string(&ll) {
                    let mut rec = Rec::default();
                    scan_ir(&ir, &mut rec);
                    per_target.insert(e.file_name().to_string_lossy().into_owned(), if rec.cc.is_empty() { ["ccc".to_owned()].into() } else { rec.cc });
                }
            }
        }
        let mut by_cc: BTreeMap<String, usize> = BTreeMap::new();
        for s in per_target.values() {
            for c in s {
                *by_cc.entry(c.clone()).or_default() += 1;
            }
        }
        let _ = writeln!(out, "  extern \"C\" lowering in abi-diff's IR: {} targets; conventions used: {by_cc:?}", per_target.len());
    }
    print!("{out}");
    if let Some(dir) = &args.out {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("gaps-codegen.md"), gaps)?;
    }
    Ok(ExitCode::SUCCESS)
}

//! Diagnostic invariants: what every diagnostic rustc prints must satisfy, whatever the program.
//!
//! Compiles each standalone UI test with `--error-format=json` and checks every diagnostic,
//! children and suggestions included:
//!
//! - internal: user-facing text (message, labels, suggested code) contains compiler-internal
//!   debug output (`DefId(`, region and type-variable debug names, `Opaque(DefId`, `{closure#0}`
//!   in suggested code)
//! - span: a span outside its file (offsets past the end, lines past the last, start after end)
//!
//! Errors without any span and exact duplicates are notes. Tests that ask for compiler internals
//! on purpose (verbose printing, dump attributes) are left out.
//!
//! With --compiler-checks the compile is an incremental session with the patched compiler's
//! checks on: stale or order-dependent query values and new untracked reads are findings too.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::compiler_checks::{self, Known};
use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{self, Compile, Status};
use mirth_lab::uitest::{self, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    #[command(flatten)]
    sweep: Sweep,
}

static INTERNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"DefId\(|\bRe(LateParam|Bound|Var|Early|Static)\b|'\{erased\}|\?\d+[tif]\b|'\^\d+(_\d+)?\b|Opaque\(DefId|\bAlias\((Projection|Opaque|Inherent|Free)|\bBoundRegionKind|\bDefPath\b|\bLocalDefId\b|\bTyKind::").unwrap()
});
static INTERNAL_CODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{closure#\d+\}|\{opaque#\d+\}|\{async block@|\{impl#\d+\}|\{constant#\d+\}").unwrap());
static DEBUG_TEST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#!?\[rustc_(dump|effective_visibility|regions|variance|outlives|layout|abi|def_path|symbol_name|object_lifetime_default|evaluate_where_clauses|then_this_would_need|if_this_changed|clean|partition)").unwrap()
});
static DEBUG_FLAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"verbose|-Zdump|unpretty|print-").unwrap());
static KNOWN: LazyLock<Known> = LazyLock::new(Known::load);
static SUMMARY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(aborting due to|could not compile|\d+ (previous )?errors?)").unwrap());

#[derive(Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    what: String,
    detail: String,
    code: String,
}

#[derive(Serialize)]
struct Rec {
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    found: Vec<String>,
    notes: Vec<String>,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        self.found.clone()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

fn file_info(cache: &mut BTreeMap<String, Option<(usize, usize)>>, test_dir: &Path, name: &str) -> Option<(usize, usize)> {
    *cache.entry(name.to_owned()).or_insert_with(|| {
        let p = Path::new(name);
        let data = std::fs::read(if p.is_absolute() { p.to_path_buf() } else { test_dir.join(p) }).ok()?;
        Some((data.len(), data.iter().filter(|&&b| b == b'\n').count() + 1))
    })
}

fn check(args: &Args, test: &Test) -> Rec {
    let dir = driver::scratch_dir(&args.sweep);
    let mut compile = Compile::new(&args.rustc, &test.path, dir.path(), &test.flags, test.edition()).emit("metadata").json().timeout(120);
    if args.sweep.compiler_checks {
        compile = compile.compiler_checks(&dir.path().join("incr"), true);
    }
    let c = compile.run();
    let mut rec = Rec { test: test.rel.clone(), skip: None, found: Vec::new(), notes: Vec::new() };
    if matches!(c.status, Status::Ice | Status::Timeout) {
        rec.skip = Some(format!("{:?}", c.status).to_lowercase());
        return rec;
    }
    let diags = rustc::diagnostics(&c.stderr);
    let test_dir = test.path.parent().unwrap_or(Path::new("."));
    let mut cache = BTreeMap::new();
    let mut found: BTreeSet<Finding> = BTreeSet::new();
    let mut seen: BTreeMap<(String, String, String, Option<(String, usize, usize)>), usize> = BTreeMap::new();
    for diag in &diags {
        let prim = diag.primary().map(|s| (s.file_name.clone(), s.byte_start, s.byte_end));
        *seen.entry((diag.level.clone(), diag.code().to_owned(), diag.message.clone(), prim)).or_default() += 1;
        if diag.level == "error" && diag.spans.is_empty() && diag.children.is_empty() && !SUMMARY.is_match(&diag.message) {
            rec.notes.push(format!("nowhere: {}", diag.message.chars().take(100).collect::<String>()));
        }
        for node in diag.walk() {
            let texts = std::iter::once(node.message.as_str()).chain(node.spans.iter().filter_map(|s| s.label.as_deref()));
            for t in texts {
                if let Some(m) = INTERNAL.find(t) {
                    found.insert(Finding { what: "internal".into(), detail: format!("{} :: {}", m.as_str(), t.chars().take(300).collect::<String>()), code: diag.code().into() });
                }
            }
            for s in &node.spans {
                if let Some(repl) = &s.suggested_replacement
                    && let Some(m) = INTERNAL.find(repl).or_else(|| INTERNAL_CODE.find(repl))
                {
                    found.insert(Finding { what: "internal".into(), detail: format!("{} :: suggests {:?}", m.as_str(), repl.chars().take(200).collect::<String>()), code: diag.code().into() });
                }
                if s.file_name.starts_with('<') {
                    continue;
                }
                if let Some((size, lines)) = file_info(&mut cache, test_dir, &s.file_name)
                    && (s.byte_start > s.byte_end || s.byte_end > size || s.line_start > lines || s.line_end > lines || s.line_start > s.line_end)
                {
                    found.insert(Finding { what: "span".into(), detail: format!("{}:{}..{} lines {}..{} (file: {size} bytes, {lines} lines): {}", s.file_name, s.byte_start, s.byte_end, s.line_start, s.line_end, node.message.chars().take(150).collect::<String>()), code: diag.code().into() });
                }
            }
        }
    }
    for ((level, _, message, _), n) in &seen {
        if *n > 1 && (level == "error" || level == "warning") {
            rec.notes.push(format!("duplicate x{n}: {}", message.chars().take(100).collect::<String>()));
        }
    }
    if args.sweep.compiler_checks {
        let report = compiler_checks::read(&c.stderr, &KNOWN);
        rec.notes.extend(report.notes.iter().map(|n| if n.starts_with("known reuse") { n.clone() } else { format!("untracked site: {n}") }));
        for f in report.findings() {
            let (what, detail) = f.split_once(": ").unwrap_or(("compiler-check", &f));
            found.insert(Finding { what: what.into(), detail: detail.into(), code: String::new() });
        }
    }
    rec.found = found.iter().map(|f| format!("{}: {}", f.what, f.detail.chars().take(200).collect::<String>())).collect();
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &[], &serde_json::json!({ "found": found, "notes": rec.notes }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let tests = uitest::tests(&args.sweep.tests, uitest::ALL, |t| {
        uitest::flag_matches(t, &DEBUG_FLAG) || DEBUG_TEST.is_match(&t.text) || t.text.contains("assumptions_on_binders")
    });
    let tests = args.sweep.select(tests);
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, t)))
}

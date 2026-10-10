//! Feature gates: nothing unstable may be usable from stable code, whatever the spelling.
//!
//! Compiled without `#![feature]` and without `RUSTC_BOOTSTRAP`, as a stable user would:
//!
//! - attributes: every attribute in the "Unstable attributes" part of
//!   compiler/rustc_feature/src/builtin_attrs.rs, on each kind of item and position
//! - library: every top-level `pub` item of core, alloc and std marked
//!   `#[unstable(feature)]`, reached by `use` (direct, renamed, glob), implemented (traits),
//!   taken as a value (functions) or named as a type
//!
//! A program must report the gate. One that compiles is a finding; one that fails without
//! mentioning the gate is noted. Library items whose path does not resolve even with the
//! feature are skipped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::rustc::{Compile, Status};
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// The rust checkout (for the attribute table and the library sources).
    #[arg(long)]
    rust: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value_t = 8)]
    jobs: usize,
    /// `attributes` or `library`.
    #[arg(long)]
    only: Option<String>,
}

static GATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"E0658|is experimental|is unstable|unstable feature|use of unstable|internal implementation detail|unstable library feature|requires a nightly|used internally by the standard library|may not be used|are considered unstable|is an internal|cannot be used on stable").unwrap()
});
/// Arguments for attributes that need them; the rest are written bare.
const ATTR_ARGS: &[(&str, &str)] = &[
    ("optimize", "(speed)"), ("patchable_function_entry", "(prefix_nops = 1, entry_nops = 1)"),
    ("instrument_fn", " = \"on\""), ("cfi_encoding", " = \"u1x\""), ("register_tool", "(mytool)"),
    ("register_attribute_tool", "(mytool)"), ("register_lint_tool", "(mytool)"), ("linkage", " = \"weak\""),
    ("lang", " = \"mirth_nonexistent\""), ("rustc_on_unimplemented", "(message = \"x\")"),
    ("rustc_diagnostic_item", " = \"mirth_x\""), ("test_runner", "(crate::r)"), ("pattern_complexity_limit", " = 10"),
    ("rustc_legacy_const_generics", "(0)"), ("rustc_abi", "(debug)"), ("rustc_macro_transparency", " = \"semitransparent\""),
    ("unstable", "(feature = \"x\", issue = \"none\")"), ("stable", "(feature = \"x\", since = \"1.0.0\")"),
    ("rustc_const_unstable", "(feature = \"x\", issue = \"none\")"), ("rustc_const_stable", "(feature = \"x\", since = \"1.0.0\")"),
    ("feature", "(mirth_nonexistent)"), ("rustc_objc_class", " = \"X\""), ("rustc_objc_selector", " = \"x\""),
    ("rustc_confusables", "(\"x\")"), ("rustc_must_implement_one_of", "(a, b)"), ("allow_internal_unstable", "(core_intrinsics)"),
    ("rustc_allow_const_fn_unstable", "(x)"), ("rustc_default_body_unstable", "(feature = \"x\", issue = \"none\")"),
    ("rustc_simd_monomorphize_lane_limit", " = \"8\""), ("rustc_scalable_vector", "(4)"),
];
/// Each position: a name, and a program with `{A}` where the outer attribute goes (`{AI}`:
/// the inner one).
const POSITIONS: &[(&str, &str)] = &[
    ("fn", "{A}\npub fn f() {}\nfn main() {}"),
    ("fn-no-body", "pub trait T { {A} fn m(&self); }\nfn main() {}"),
    ("foreign-fn", "unsafe extern \"C\" { {A} fn ext(); }\nfn main() {}"),
    ("param", "pub fn f({A} x: u32) -> u32 { x }\nfn main() {}"),
    ("param-no-body", "pub trait T { fn m(&self, {A} x: u32); }\nfn main() {}"),
    ("fn-ptr-param", "pub type F = fn({A} u32);\nfn main() {}"),
    ("struct", "{A}\npub struct S;\nfn main() {}"),
    ("field", "pub struct S { {A} pub x: u32 }\nfn main() {}"),
    ("impl", "pub struct S;\n{A}\nimpl S {}\nfn main() {}"),
    ("trait", "{A}\npub trait T {}\nfn main() {}"),
    ("mod", "{A}\npub mod m {}\nfn main() {}"),
    ("closure", "fn main() { let _c = {A} || (); }"),
    ("statement", "fn main() { {A} let _x = 1; }"),
    ("crate", "#![{AI}]\nfn main() {}"),
];

#[derive(Serialize)]
struct Res {
    kind: &'static str,
    item: String,
    position: String,
    result: String,
    first: String,
}

fn compile_text(args: &Args, text: &str, feature: Option<&str>) -> (Status, String) {
    let dir = tempfile::tempdir_in(&args.work).expect("scratch");
    let f = dir.path().join("t.rs");
    let body = match feature {
        Some(feat) => format!("#![feature({feat})]\n{text}"),
        None => text.to_owned(),
    };
    let _ = std::fs::write(&f, body);
    let mut c = Compile::new(&args.rustc, &f, dir.path(), &[], "2021").emit("metadata").timeout(60);
    if feature.is_none() {
        c = c.stable();
    }
    let c = c.run();
    (c.status, c.stderr)
}

fn classify(status: Status, stderr: &str, item: &str) -> String {
    // `#[feature]` outside the crate root does nothing; rustc warns that it belongs at the root.
    if status == Status::Ok && item == "feature" && stderr.contains("crate-level attribute") {
        return "gated".into();
    }
    match status {
        Status::Ok => "accepted".into(),
        Status::Ice => "ice".into(),
        _ if GATE.is_match(stderr) => "gated".into(),
        _ => "no-gate-message".into(),
    }
}

fn first_error(stderr: &str) -> String {
    stderr.lines().find(|l| l.starts_with("error")).unwrap_or("").chars().take(200).collect()
}

fn attributes(args: &Args) -> Vec<Res> {
    let table = std::fs::read_to_string(args.rust.join("compiler/rustc_feature/src/builtin_attrs.rs")).unwrap_or_default();
    let start = table.find("Unstable attributes:").unwrap_or(0);
    let re = Regex::new(r"sym::([a-z_0-9]+)").unwrap();
    let mut names: Vec<String> = re.captures_iter(&table[start..]).map(|c| c[1].to_owned()).collect();
    names.sort();
    names.dedup();
    let args_of: BTreeMap<&str, &str> = ATTR_ARGS.iter().copied().collect();
    let jobs: Vec<(String, &str, String)> = names
        .iter()
        .flat_map(|name| {
            let a = args_of.get(name.as_str()).copied().unwrap_or("");
            POSITIONS.iter().map(move |(pos, t)| {
                (name.clone(), *pos, t.replace("{AI}", &format!("{name}{a}")).replace("{A}", &format!("#[{name}{a}]")))
            })
        })
        .collect();
    jobs.par_iter()
        .map(|(name, pos, prog)| {
            let (status, err) = compile_text(args, prog, None);
            Res { kind: "attribute", item: name.clone(), position: pos.to_string(), result: classify(status, &err, name), first: first_error(&err) }
        })
        .collect()
}

struct Item {
    path: String,
    kind: String,
    feature: String,
    generic: bool,
}

fn library_items(rust: &Path) -> Vec<Item> {
    let unstable = Regex::new(r#"^#\[unstable\(feature = "([a-z_0-9]+)""#).unwrap();
    let decl = Regex::new(r#"^pub (?:const |unsafe |auto |extern "C" )*(struct|enum|trait|union|type|fn|const|static|macro) ([A-Za-z_][A-Za-z_0-9]*)(<)?"#).unwrap();
    let mut items = Vec::new();
    for krate in ["core", "alloc", "std"] {
        let root = rust.join("library").join(krate).join("src");
        for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
            let p = entry.path();
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let rel = p.strip_prefix(&root).unwrap().with_extension("");
            let mut module = vec![krate.to_owned()];
            module.extend(rel.iter().map(|c| c.to_string_lossy().into_owned()).filter(|c| c != "lib" && c != "mod"));
            let Ok(text) = std::fs::read_to_string(p) else { continue };
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let Some(m) = unstable.captures(line) else { continue };
                for next in lines.iter().skip(i + 1).take(5) {
                    if next.starts_with('#') {
                        continue;
                    }
                    if let Some(d) = decl.captures(next) {
                        items.push(Item { path: format!("{}::{}", module.join("::"), &d[2]), kind: d[1].to_owned(), feature: m[1].to_owned(), generic: d.get(3).is_some() });
                    }
                    break;
                }
            }
        }
    }
    items
}

fn library(args: &Args) -> Vec<Res> {
    library_items(&args.rust)
        .par_iter()
        .flat_map(|item| {
            let (status, _) = compile_text(args, &format!("#[allow(unused_imports)] use {};\nfn main() {{}}", item.path), Some(&item.feature));
            if status != Status::Ok {
                return vec![Res { kind: "library", item: item.path.clone(), position: "path".into(), result: "skipped: path".into(), first: String::new() }];
            }
            let (parent, name) = item.path.rsplit_once("::").unwrap();
            let p = &item.path;
            let mut progs = vec![
                ("use", format!("#[allow(unused_imports)] use {p};\nfn main() {{}}")),
                ("use-as", format!("#[allow(unused_imports)] use {p} as Renamed;\nfn main() {{}}")),
                ("glob", format!("#[allow(unused_imports)] use {parent}::*;\n#[allow(unused_imports)] use self::{name} as _;\nfn main() {{}}")),
            ];
            if item.kind == "trait" && !item.generic {
                progs.push(("impl", format!("struct L;\nimpl {p} for L {{}}\nfn main() {{}}")));
            }
            if item.kind == "fn" && !item.generic {
                progs.push(("value", format!("fn main() {{ let _f = {p}; }}")));
            }
            if ["struct", "enum", "union", "type"].contains(&item.kind.as_str()) && !item.generic {
                progs.push(("type", format!("pub fn g(_: Option<&{p}>) {{}}\nfn main() {{}}")));
            }
            progs
                .into_iter()
                .map(|(pos, prog)| {
                    let (status, err) = compile_text(args, &prog, None);
                    Res { kind: "library", item: item.path.clone(), position: pos.into(), result: classify(status, &err, ""), first: first_error(&err) }
                })
                .collect()
        })
        .collect()
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build_global().ok();
    let mut results = Vec::new();
    if args.only.as_deref().is_none_or(|o| o == "attributes") {
        results.extend(attributes(&args));
    }
    if args.only.as_deref().is_none_or(|o| o == "library") {
        results.extend(library(&args));
    }
    std::fs::write(args.work.join("results.json"), serde_json::to_string_pretty(&results)?)?;
    let mut counts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for r in &results {
        *counts.entry((r.kind, r.result.as_str())).or_default() += 1;
    }
    println!("{counts:?}");
    for r in results.iter().filter(|r| r.result == "accepted" || r.result == "ice") {
        println!("{:9} {:9} {:45} {}", r.result.to_uppercase(), r.kind, r.item, r.position);
    }
    Ok(ExitCode::SUCCESS)
}

//! rustdoc consistency: rustdoc is a second front end over the same compiler, so it must agree
//! with rustc about the programs both read (check 6 of docs/checks.md).
//!
//! For each standalone UI test:
//!
//! - ice: rustdoc panics, on any test (also on programs rustc rejects).
//! - rejects: rustc accepts the test (`--emit=metadata`) and rustdoc reports an error, in HTML,
//!   in HTML with `--document-private-items`, or in JSON (`--output-format json`). rustdoc
//!   accepting what rustc rejects is expected: it does not type-check function bodies.
//! - json: the JSON does not hold together: the root, an id a module, struct, enum, trait or impl
//!   lists, or an intra-doc link names, is missing from the index; an `id` in a type or a `use`
//!   is in neither the index nor `paths`.
//! - reexport: a `pub use` at the crate root (not glob, not `_`, not `#[doc(hidden)]`) is in
//!   neither form in the root module of the public JSON: as a `use` item of that name, or the
//!   item itself inlined under that name.
//! - auto-trait: for each auto-trait impl rustdoc synthesizes (Send, Sync, Unpin, UnwindSafe,
//!   RefUnwindSafe), a probe appended to the test requires the trait for the type under exactly
//!   the bounds rustdoc shows; it must compile (#162274). For a negative impl the probe must
//!   fail with E0277. A probe that fails for another reason (a type that cannot be named from
//!   the crate root, a `#![forbid]`) is a note.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;
use std::time::Duration;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{self, Compile, Exit, Status, is_ice, run_command};
use mirth_lab::uitest::{self, Kind, Test};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The toolchain whose rustc and rustdoc to compare (they must be the same commit).
    #[arg(long, default_value = "nightly-2026-10-06")]
    toolchain: String,
    #[command(flatten)]
    sweep: Sweep,
}

/// Tests whose flags make no sense to rustdoc, or ask rustc for something rustdoc does not do.
/// Matched against the flags joined by spaces (`-Z x` is two tokens). `-Zparse-crate-root-only`
/// stops rustc after parsing, so its "accepts" means "parses".
static SKIP_FLAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^| )(--test|--print|-Z ?unpretty|--emit|-o( |$)|--out-dir|-Z ?no-codegen|-Z ?parse-crate-root-only|-Z ?parse-only)").unwrap()
});
/// Flags rustdoc does not take; left out (the outcome cannot depend on them).
static DROP_FLAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(-O|-g)$").unwrap());
static CRATE_TYPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"crate_type|crate-type").unwrap());
static CRATE_TYPE_ATTR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?m)^\s*#!\[crate_type\s*=\s*"([a-z-]+)"\]"#).unwrap());
static UNKNOWN_OPTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)unrecognized option|unknown codegen option|unknown unstable option|requires an argument").unwrap());

/// Expected differences between rustdoc and rustc, by the error's code or lint, with why.
const NOISE: &[(&str, &str)] = &[
    // rustdoc's own lints (broken intra-doc links, invalid HTML) under the test's
    // `#![deny(warnings)]`: rustc does not run them.
    ("rustdoc::", "a rustdoc lint denied by the test"),
];

/// rustdoc ICEs already reported upstream: a line of the panic, and the issue.
const KNOWN_ICE: &[(&str, &str)] = &[
    ("cx.impl_trait_bounds.is_empty()", "rust-lang/rust#155728 (fn_delegation)"),
    // `UnsafeBinder(_) => unimplemented!()` under `FIXME(unsafe_binder): Implement rustdoc-json`.
    ("json/conversions.rs:706:32", "FIXME(unsafe_binder) in rustdoc JSON"),
];

const AUTO_TRAITS: &[&str] = &["Send", "Sync", "Unpin", "UnwindSafe", "RefUnwindSafe"];

#[derive(Serialize)]
struct Rec {
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    rustc: Option<Status>,
    found: Vec<String>,
    notes: Vec<String>,
    probes: usize,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        self.found.clone()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

struct Tools {
    rustc: PathBuf,
    rustdoc: PathBuf,
}

struct Doc {
    status: Status,
    stderr: String,
    json: Option<Value>,
}

fn rustdoc(tools: &Tools, test: &Test, flags: &[String], out: &Path, extra: &[&str]) -> Doc {
    let mut cmd = Command::new(&tools.rustdoc);
    cmd.arg(&test.path)
        .args(["--edition", test.edition(), "-Zunstable-options", "--error-format=json", "-Ainternal_features", "-Aincomplete_features"])
        .arg("-o")
        .arg(out)
        .args(flags)
        .args(extra)
        .current_dir(out.parent().unwrap_or(out))
        .env("RUSTC_BOOTSTRAP", "1")
        .env("RUST_BACKTRACE", "0");
    let done = match run_command(cmd, Duration::from_secs(120)) {
        Ok(d) => d,
        Err(e) => return Doc { status: Status::Error, stderr: e.to_string(), json: None },
    };
    let stderr = done.stderr_text();
    let status = match done.exit {
        Exit::Timeout => Status::Timeout,
        _ if is_ice(&stderr) => Status::Ice,
        Exit::Code(0) => Status::Ok,
        _ => Status::Error,
    };
    let json = (status == Status::Ok && extra.contains(&"json"))
        .then(|| {
            let name = std::fs::read_dir(out).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "json"))?;
            serde_json::from_slice(&std::fs::read(name).ok()?).ok()
        })
        .flatten();
    Doc { status, stderr, json }
}

/// The errors' codes or lint names, for the noise list.
fn error_names(stderr: &str) -> Vec<String> {
    rustc::diagnostics(stderr)
        .iter()
        .filter(|d| d.level == "error" && !d.message.starts_with("aborting due to"))
        .map(|d| {
            if !d.code().is_empty() {
                return d.code().to_owned();
            }
            // A denied lint names itself in a child note: "`#[deny(rustdoc::x)]` implied by ...".
            d.children.iter().find_map(|c| c.message.split('`').nth(1).map(|s| s.trim_start_matches("#[deny(").trim_end_matches(")]").to_owned())).unwrap_or_else(|| d.message.chars().take(80).collect())
        })
        .collect()
}

fn noise(names: &[String]) -> Option<&'static str> {
    if names.is_empty() {
        return None;
    }
    NOISE.iter().find(|(pat, _)| names.iter().all(|n| n.contains(pat))).map(|(_, why)| *why)
}

// ---- JSON self-consistency ----

fn id_of(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Ids a container lists, which must be items of the index.
const LISTS: &[&str] = &["items", "fields", "variants", "impls", "implementations", "tuple"];

fn walk_ids(v: &Value, key: &str, index: &serde_json::Map<String, Value>, paths: &serde_json::Map<String, Value>, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == "id"
                    && let Some(id) = id_of(x)
                    && !index.contains_key(&id)
                    && !paths.contains_key(&id)
                {
                    out.insert(format!("id {id} (in {key}) in neither index nor paths"));
                }
                walk_ids(x, k, index, paths, out);
            }
        }
        Value::Array(a) => {
            for x in a {
                if LISTS.contains(&key)
                    && let Some(id) = id_of(x)
                    && !index.contains_key(&id)
                {
                    out.insert(format!("{key} lists id {id}, not in the index"));
                }
                walk_ids(x, key, index, paths, out);
            }
        }
        _ => {}
    }
}

/// The items reachable from the root through any id the index holds (jsondoclint's walk).
fn reachable(doc: &Value) -> BTreeSet<String> {
    fn ids(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => m.iter().for_each(|(k, x)| {
                if k == "id"
                    && let Some(id) = id_of(x)
                {
                    out.push(id);
                }
                ids(x, out)
            }),
            Value::Array(a) => a.iter().for_each(|x| {
                if let Some(id) = id_of(x) {
                    out.push(id);
                }
                ids(x, out)
            }),
            _ => {}
        }
    }
    let index = &doc["index"];
    let mut seen = BTreeSet::new();
    let mut todo: Vec<String> = id_of(&doc["root"]).into_iter().collect();
    while let Some(id) = todo.pop() {
        if index.get(&id).is_none() || !seen.insert(id.clone()) {
            continue;
        }
        let mut next = Vec::new();
        ids(&index[&id]["inner"], &mut next);
        if let Some(links) = index[&id]["links"].as_object() {
            next.extend(links.values().filter_map(id_of));
        }
        todo.extend(next);
    }
    seen
}

/// Problems in the items reachable from the root, and how many unreachable items have them.
fn consistency(doc: &Value) -> (Vec<String>, usize) {
    let empty = serde_json::Map::new();
    let index = doc["index"].as_object().unwrap_or(&empty);
    let paths = doc["paths"].as_object().unwrap_or(&empty);
    let live = reachable(doc);
    let mut out = BTreeSet::new();
    let mut orphans = 0;
    if let Some(root) = id_of(&doc["root"])
        && !index.contains_key(&root)
    {
        out.insert(format!("root {root} not in the index"));
    }
    for (id, item) in index {
        let mut problems = BTreeSet::new();
        walk_ids(&item["inner"], "inner", index, paths, &mut problems);
        if let Some(links) = item["links"].as_object() {
            for (text, target) in links {
                if let Some(t) = id_of(target)
                    && !index.contains_key(&t)
                    && !paths.contains_key(&t)
                {
                    problems.insert(format!("link {text:?} -> {t} in neither index nor paths"));
                }
            }
        }
        if live.contains(id) {
            out.extend(problems);
        } else if !problems.is_empty() {
            orphans += 1;
        }
    }
    (out.into_iter().collect(), orphans)
}

// ---- re-exports ----

/// `#[doc(hidden)]`, or conditional: rustdoc may rightly leave it out.
fn doc_hidden(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| match &a.meta {
        syn::Meta::List(l) if a.path().is_ident("doc") => l.tokens.to_string().contains("hidden"),
        _ => a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"),
    })
}

fn use_names(tree: &syn::UseTree, out: &mut Vec<String>) {
    match tree {
        syn::UseTree::Path(p) => use_names(&p.tree, out),
        syn::UseTree::Name(n) if n.ident != "self" => out.push(n.ident.to_string()),
        syn::UseTree::Rename(r) if r.rename != "_" => out.push(r.rename.to_string()),
        syn::UseTree::Group(g) => g.items.iter().for_each(|t| use_names(t, out)),
        _ => {}
    }
}

/// Names the crate root re-exports publicly, from the source.
fn root_reexports(text: &str) -> Vec<String> {
    let Ok(file) = syn::parse_file(text) else { return Vec::new() };
    let mut out = Vec::new();
    for item in &file.items {
        if let syn::Item::Use(u) = item
            && matches!(u.vis, syn::Visibility::Public(_))
            && !doc_hidden(&u.attrs)
        {
            use_names(&u.tree, &mut out);
        }
    }
    out
}

fn reexport_missing(doc: &Value, names: &[String]) -> Vec<String> {
    let root = id_of(&doc["root"]).unwrap_or_default();
    let index = &doc["index"];
    let present: HashSet<String> = index[&root]["inner"]["module"]["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| index.get(id_of(id)?))
        .filter_map(|item| item["inner"]["use"]["name"].as_str().or_else(|| item["name"].as_str()).map(str::to_owned))
        .collect();
    names.iter().filter(|n| !present.contains(*n)).map(|n| format!("`pub use … {n}` not in the root module")).collect()
}

// ---- auto-trait probes ----

struct Render<'a> {
    paths: &'a Value,
    ok: bool,
}

impl Render<'_> {
    fn path(&mut self, p: &Value) -> String {
        let id = id_of(&p["id"]).unwrap_or_default();
        let base = match self.paths[&id]["path"].as_array() {
            Some(segs) => {
                let segs: Vec<&str> = segs.iter().filter_map(Value::as_str).collect();
                let crate_id = self.paths[&id]["crate_id"].as_u64().unwrap_or(1);
                match (crate_id, segs.split_first()) {
                    (0, Some((_, rest))) => format!("crate::{}", rest.join("::")),
                    (_, Some((_, rest))) if rest.last().is_some_and(|l| AUTO_TRAITS.contains(l) || *l == "Sized") => {
                        let l = rest.last().unwrap();
                        if l.ends_with("UnwindSafe") { format!("std::panic::{l}") } else { format!("std::marker::{l}") }
                    }
                    (_, Some((krate, rest))) if matches!(*krate, "core" | "alloc" | "std") => {
                        // The canonical path can go through private modules (core::ops::function):
                        // std re-exports nearly everything at its second level.
                        match rest {
                            [m, .., last] if rest.len() > 2 => format!("std::{m}::{last}"),
                            _ => format!("std::{}", rest.join("::")),
                        }
                    }
                    (_, Some((krate, rest))) => format!("::{krate}::{}", rest.join("::")),
                    _ => {
                        self.ok = false;
                        String::new()
                    }
                }
            }
            None => p["path"].as_str().unwrap_or("").to_owned(),
        };
        format!("{base}{}", self.args(&p["args"]))
    }

    fn args(&mut self, a: &Value) -> String {
        if let Some(ab) = a.get("angle_bracketed") {
            let mut parts: Vec<String> = ab["args"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|g| {
                    if let Some(l) = g.get("lifetime") {
                        l.as_str().unwrap_or("'_").to_owned()
                    } else if let Some(t) = g.get("type") {
                        self.ty(t)
                    } else if let Some(c) = g.get("const") {
                        let e = c["expr"].as_str().unwrap_or("_");
                        if e.contains(|ch: char| !ch.is_alphanumeric() && ch != '_') { format!("{{ {e} }}") } else { e.to_owned() }
                    } else {
                        "_".into()
                    }
                })
                .collect();
            for c in ab["constraints"].as_array().into_iter().flatten() {
                let name = format!("{}{}", c["name"].as_str().unwrap_or(""), self.args(&c["args"]));
                if let Some(t) = c["binding"].get("equality").and_then(|e| e.get("type")) {
                    parts.push(format!("{name} = {}", self.ty(t)));
                } else if let Some(bs) = c["binding"].get("constraint") {
                    parts.push(format!("{name}: {}", self.bounds(bs)));
                } else {
                    self.ok = false;
                }
            }
            if parts.is_empty() { String::new() } else { format!("<{}>", parts.join(", ")) }
        } else if let Some(p) = a.get("parenthesized") {
            let inputs: Vec<String> = p["inputs"].as_array().into_iter().flatten().map(|t| self.ty(t)).collect();
            let out = if p["output"].is_null() { String::new() } else { format!(" -> {}", self.ty(&p["output"])) };
            format!("({}){out}", inputs.join(", "))
        } else if a.is_null() {
            String::new()
        } else {
            self.ok = false;
            String::new()
        }
    }

    fn bound(&mut self, b: &Value) -> String {
        if let Some(t) = b.get("trait_bound") {
            let hr = self.binder(&t["generic_params"]);
            let m = match t["modifier"].as_str() {
                Some("maybe") => "?",
                Some("none") | None => "",
                _ => {
                    self.ok = false;
                    ""
                }
            };
            format!("{hr}{m}{}", self.path(&t["trait"]))
        } else if let Some(l) = b.get("outlives") {
            l.as_str().unwrap_or("'_").to_owned()
        } else {
            self.ok = false;
            String::new()
        }
    }

    fn bounds(&mut self, bs: &Value) -> String {
        let v: Vec<String> = bs.as_array().into_iter().flatten().map(|b| self.bound(b)).collect();
        v.join(" + ")
    }

    fn binder(&mut self, params: &Value) -> String {
        let ps: Vec<String> = params.as_array().into_iter().flatten().filter_map(|p| p["name"].as_str().map(str::to_owned)).collect();
        if ps.is_empty() { String::new() } else { format!("for<{}> ", ps.join(", ")) }
    }

    fn ty(&mut self, t: &Value) -> String {
        if let Some(p) = t.get("resolved_path") {
            return self.path(p);
        }
        if let Some(g) = t.get("generic").and_then(Value::as_str) {
            return g.to_owned();
        }
        if let Some(p) = t.get("primitive").and_then(Value::as_str) {
            return if p == "never" { "!".into() } else { p.to_owned() };
        }
        if let Some(ts) = t.get("tuple").and_then(Value::as_array) {
            let v: Vec<String> = ts.iter().map(|x| self.ty(x)).collect();
            return if v.len() == 1 { format!("({},)", v[0]) } else { format!("({})", v.join(", ")) };
        }
        if let Some(s) = t.get("slice") {
            return format!("[{}]", self.ty(s));
        }
        if let Some(a) = t.get("array") {
            let len = a["len"].as_str().unwrap_or("_");
            let len = if len.contains(|c: char| !c.is_alphanumeric() && c != '_') { format!("{{ {len} }}") } else { len.to_owned() };
            return format!("[{}; {len}]", self.ty(&a["type"]));
        }
        if let Some(r) = t.get("borrowed_ref") {
            let l = r["lifetime"].as_str().map(|l| format!("{l} ")).unwrap_or_default();
            let m = if r["is_mutable"].as_bool() == Some(true) { "mut " } else { "" };
            return format!("&{l}{m}{}", self.ty(&r["type"]));
        }
        if let Some(r) = t.get("raw_pointer") {
            let m = if r["is_mutable"].as_bool() == Some(true) { "mut" } else { "const" };
            return format!("*{m} {}", self.ty(&r["type"]));
        }
        if let Some(d) = t.get("dyn_trait") {
            let mut parts: Vec<String> = d["traits"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| {
                    let hr = self.binder(&p["generic_params"]);
                    format!("{hr}{}", self.path(&p["trait"]))
                })
                .collect();
            if let Some(l) = d["lifetime"].as_str() {
                parts.push(l.to_owned());
            }
            return format!("dyn {}", parts.join(" + "));
        }
        if let Some(q) = t.get("qualified_path") {
            let st = self.ty(&q["self_type"]);
            let name = format!("{}{}", q["name"].as_str().unwrap_or(""), self.args(&q["args"]));
            return if q["trait"].is_null() { format!("{st}::{name}") } else { format!("<{st} as {}>::{name}", self.path(&q["trait"])) };
        }
        if let Some(f) = t.get("function_pointer") {
            let hr = self.binder(&f["generic_params"]);
            let unsafety = if f["header"]["is_unsafe"].as_bool() == Some(true) { "unsafe " } else { "" };
            let abi = match &f["header"]["abi"] {
                Value::String(s) if s == "Rust" => String::new(),
                Value::Object(m) => m.keys().next().map(|k| format!("extern {k:?} ")).unwrap_or_default(),
                _ => String::new(),
            };
            let inputs: Vec<String> = f["sig"]["inputs"].as_array().into_iter().flatten().map(|p| self.ty(&p[1])).collect();
            let out = if f["sig"]["output"].is_null() { String::new() } else { format!(" -> {}", self.ty(&f["sig"]["output"])) };
            return format!("{hr}{unsafety}{abi}fn({}){out}", inputs.join(", "));
        }
        // impl Trait, inferred, patterns: not nameable in a probe.
        self.ok = false;
        "_".into()
    }

    fn generics(&mut self, g: &Value) -> (String, Vec<String>) {
        let mut params = Vec::new();
        let mut preds = Vec::new();
        for p in g["params"].as_array().into_iter().flatten() {
            let name = p["name"].as_str().unwrap_or("_").to_owned();
            let k = &p["kind"];
            if let Some(l) = k.get("lifetime") {
                let o: Vec<&str> = l["outlives"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                params.push(if o.is_empty() { name } else { format!("{name}: {}", o.join(" + ")) });
            } else if let Some(t) = k.get("type") {
                if t["is_synthetic"].as_bool() == Some(true) {
                    self.ok = false;
                }
                let b = self.bounds(&t["bounds"]);
                params.push(if b.is_empty() { name } else { format!("{name}: {b}") });
            } else if let Some(c) = k.get("const") {
                params.push(format!("const {name}: {}", self.ty(&c["type"])));
            }
        }
        for w in g["where_predicates"].as_array().into_iter().flatten() {
            if let Some(b) = w.get("bound_predicate") {
                let hr = self.binder(&b["generic_params"]);
                let bs = self.bounds(&b["bounds"]);
                if !bs.is_empty() {
                    preds.push(format!("{hr}{}: {bs}", self.ty(&b["type"])));
                }
            } else if let Some(r) = w.get("lifetime_predicate") {
                let o: Vec<&str> = r["outlives"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                preds.push(format!("{}: {}", r["lifetime"].as_str().unwrap_or("'_"), o.join(" + ")));
            } else {
                self.ok = false;
            }
        }
        (params.join(", "), preds)
    }
}

struct Probe {
    what: String,
    negative: bool,
    line: usize,
    tname: String,
}

/// Whether an E0277 is about the auto trait itself (its name, or its `on_unimplemented` text),
/// not about a bound the type needs to be named at all.
fn about(message: &str, tname: &str) -> bool {
    let phrase = match tname {
        "Send" => "cannot be sent between threads safely",
        "Sync" => "cannot be shared between threads safely",
        "Unpin" => "cannot be unpinned",
        "UnwindSafe" => "may not be safely transferred across an unwind boundary",
        _ => "may contain interior mutability",
    };
    message.contains(phrase) || message.contains(&format!("{tname}`")) || message.contains(&format!("{tname}>"))
}

/// The probe module starts after the test's last line.
fn in_probe_module(test: &Test, line: usize) -> bool {
    line > test.text.lines().count()
}

fn auto_trait_probes(doc: &Value, base_lines: usize) -> (String, Vec<Probe>) {
    // `use std;` makes `std::` paths work in every edition (2015 paths are module-relative), and
    // `dyn std::…` parses where `dyn ::std::…` does not in 2015. It takes no line of its own.
    let mut code = String::from("\n#[allow(warnings, clippy::all)]\nmod __mirth_probe { use std;\n");
    // The block starts with an empty line, the attribute and `mod`: the first probe is N + 4.
    let mut line = base_lines + 4;
    let mut probes = Vec::new();
    let paths = &doc["paths"];
    let mut traits_needed: BTreeSet<String> = BTreeSet::new();
    for item in doc["index"].as_object().into_iter().flat_map(|m| m.values()) {
        let Some(imp) = item["inner"].get("impl") else { continue };
        if imp["is_synthetic"].as_bool() != Some(true) || imp["trait"].is_null() {
            continue;
        }
        let tname = imp["trait"]["path"].as_str().unwrap_or("");
        if !AUTO_TRAITS.contains(&tname) {
            continue;
        }
        let mut r = Render { paths, ok: true };
        // The canonical path can go through a private module (core::panic::unwind_safe).
        let trait_path = if tname.ends_with("UnwindSafe") { format!("std::panic::{tname}") } else { format!("std::marker::{tname}") };
        let for_ty = r.ty(&imp["for"]);
        let (params, mut preds) = r.generics(&imp["generics"]);
        // rustdoc leaves the type's own bounds implied; the probe must state them to name it.
        let names: HashSet<&str> = imp["generics"]["params"].as_array().into_iter().flatten().filter_map(|p| p["name"].as_str()).collect();
        let own = id_of(&imp["for"]["resolved_path"]["id"]).and_then(|id| doc["index"].get(&id)).and_then(|t| {
            let inner = &t["inner"];
            inner.get("struct").or_else(|| inner.get("enum")).or_else(|| inner.get("union")).map(|k| k["generics"].clone())
        });
        if let Some(g) = own {
            let ps: Vec<&Value> = g["params"].as_array().into_iter().flatten().collect();
            if ps.iter().all(|p| p["name"].as_str().is_some_and(|n| names.contains(n))) {
                for p in &ps {
                    if let Some(t) = p["kind"].get("type") {
                        let b = r.bounds(&t["bounds"]);
                        if !b.is_empty() {
                            preds.push(format!("{}: {b}", p["name"].as_str().unwrap_or("_")));
                        }
                    }
                }
                let (_, wh) = r.generics(&serde_json::json!({"params": [], "where_predicates": g["where_predicates"]}));
                preds.extend(wh);
            }
        }
        if !r.ok || !for_ty.starts_with("crate::") {
            continue;
        }
        let negative = imp["is_negative"].as_bool() == Some(true);
        let n = probes.len();
        let req = format!("__req_{}", tname);
        traits_needed.insert(format!("    fn {req}<X: ?Sized + {trait_path}>() {{}}\n"));
        let wh = if preds.is_empty() { String::new() } else { format!(" where {}", preds.join(", ")) };
        let what = format!("{}impl<{params}> {tname} for {for_ty}{wh}", if negative { "!" } else { "" });
        let tname = tname.to_owned();
        code.push_str(&format!("    fn __p{n}<{params}>(){wh} {{ {req}::<{for_ty}>(); }}\n"));
        probes.push(Probe { what, negative, line, tname });
        line += 1;
    }
    if probes.is_empty() {
        return (String::new(), probes);
    }
    for t in traits_needed {
        code.push_str(&t);
    }
    code.push_str("}\n");
    (code, probes)
}

/// Compile the test with the probes appended; findings and notes per probe.
fn run_probes(tools: &Tools, test: &Test, dir: &Path, code: &str, probes: &[Probe], found: &mut Vec<String>, notes: &mut Vec<String>) -> Option<Vec<u8>> {
    let src = dir.join(test.file_name());
    let mut text = test.text.clone();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(code);
    std::fs::write(&src, &text).ok()?;
    let out = dir.join("probe-out");
    let c = Compile::new(&tools.rustc, &src, &out, &test.flags, test.edition()).emit("metadata").json().timeout(120).run();
    let diags = rustc::diagnostics(&c.stderr);
    let mut e0277: Vec<(usize, String)> = Vec::new();
    let mut other: BTreeSet<usize> = BTreeSet::new();
    let mut outside = false;
    for d in diags.iter().filter(|d| d.level == "error") {
        let line = d.primary().map(|s| s.line_start).unwrap_or(0);
        if probes.iter().all(|p| p.line != line) {
            if d.message.starts_with("aborting") {
                continue;
            }
            if in_probe_module(test, line) {
                notes.push(format!("probe module broken: {}", d.message.chars().take(120).collect::<String>()));
                return None;
            }
            outside = true;
            continue;
        }
        if d.code() == "E0277" {
            e0277.push((line, d.message.clone()));
        } else {
            other.insert(line);
        }
    }
    // Any other error (resolution, privacy) can stop rustc before it checks the probes at all:
    // then a missing E0277 proves nothing.
    if !other.is_empty() && probes.iter().any(|p| p.negative) {
        notes.push("probe module broken: no verdict for the negative impls".into());
        return None;
    }
    if outside || c.status == Status::Ice || c.status == Status::Timeout {
        let first = diags.iter().find(|d| d.level == "error").map_or(String::new(), |d| d.message.chars().take(120).collect());
        notes.push(format!("probe compile failed outside the probes ({:?}): {first}", c.status));
        return None;
    }
    let mut bad = false;
    for p in probes {
        let mine: Vec<&String> = e0277.iter().filter(|(l, _)| *l == p.line).map(|(_, m)| m).collect();
        let fails = mine.iter().any(|m| about(m, &p.tname));
        if other.contains(&p.line) || (!fails && !mine.is_empty()) {
            notes.push(format!("probe broken: {}", p.what));
        } else if p.negative && !fails {
            found.push(format!("auto-trait: holds although rustdoc shows {}", p.what));
            bad = true;
        } else if !p.negative && fails {
            found.push(format!("auto-trait: does not hold under rustdoc's bounds: {}", p.what));
            bad = true;
        }
    }
    bad.then(|| text.into_bytes())
}

fn check(args: &Args, tools: &Tools, test: &Test) -> Rec {
    let mut rec = Rec { test: test.rel.clone(), skip: None, rustc: None, found: Vec::new(), notes: Vec::new(), probes: 0 };
    if SKIP_FLAG.is_match(&test.flags.join(" ")) {
        rec.skip = Some("flags".into());
        return rec;
    }
    let mut flags: Vec<String> = test.flags.iter().filter(|f| !DROP_FLAG.is_match(f)).cloned().collect();
    // rustc's default crate type is bin, rustdoc's lib; and rustdoc does not read
    // `#![crate_type]` (Cargo always passes --crate-type), so pass what rustc would use.
    if !test.flags.iter().any(|f| CRATE_TYPE.is_match(f)) {
        let attr: Vec<String> = CRATE_TYPE_ATTR.captures_iter(&test.text).map(|c| c[1].to_owned()).collect();
        if attr.is_empty() && !CRATE_TYPE.is_match(&test.text) {
            flags.extend(["--crate-type".into(), "bin".into()]);
        }
        for t in attr {
            flags.extend(["--crate-type".into(), t]);
        }
    }
    let dir = driver::scratch_dir(&args.sweep);
    let c = Compile::new(&tools.rustc, &test.path, &dir.path().join("rustc"), &test.flags, test.edition()).emit("metadata").timeout(120).run();
    rec.rustc = Some(c.status);
    if c.status == Status::Timeout {
        rec.skip = Some("rustc timeout".into());
        return rec;
    }
    let accepted = c.status == Status::Ok && test.kind.is_none_or(|k| !matches!(k, Kind::CheckFail | Kind::BuildFail));
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let runs: &[(&str, &[&str])] = if accepted {
        &[("html", &[]), ("html-private", &["--document-private-items"]), ("json", &["--output-format", "json"]), ("json-private", &["--output-format", "json", "--document-private-items"])]
    } else {
        &[("html", &[])]
    };
    let mut public_json = None;
    for (name, extra) in runs {
        let out = dir.path().join(name);
        let d = rustdoc(tools, test, &flags, &out, extra);
        if d.status == Status::Error && UNKNOWN_OPTION.is_match(&d.stderr) {
            rec.skip = Some(format!("rustdoc option: {}", rustc::first_error(&d.stderr)));
            return rec;
        }
        match d.status {
            Status::Ice => {
                match KNOWN_ICE.iter().find(|(pat, _)| d.stderr.contains(pat)) {
                    Some((_, issue)) => rec.notes.push(format!("ice ({name}): known, {issue}")),
                    None => {
                        rec.found.push(format!("ice ({name}): {}", ice_line(&d.stderr)));
                        files.push((format!("rustdoc-{name}.stderr"), d.stderr.into_bytes()));
                    }
                }
                break;
            }
            Status::Timeout => rec.notes.push(format!("rustdoc timeout ({name})")),
            Status::Error if accepted => {
                let names = error_names(&d.stderr);
                // A test that sets a tiny recursion limit: rustdoc's extra trait work (blanket
                // impls over std types) passes a depth rustc happens to stay under.
                let overflow_only = test.text.contains("#![recursion_limit") && d.stderr.contains("overflow");
                match noise(&names).or(overflow_only.then_some("overflow under the test's recursion_limit")) {
                    Some(why) => rec.notes.push(format!("{name}: {why}")),
                    None => {
                        rec.found.push(format!("rejects ({name}): {}", names.join(", ").chars().take(150).collect::<String>()));
                        files.push((format!("rustdoc-{name}.stderr"), d.stderr.into_bytes()));
                    }
                }
            }
            _ => {}
        }
        if let Some(j) = d.json {
            let (problems, orphans) = consistency(&j);
            if orphans > 0 {
                rec.notes.push(format!("json ({name}): {orphans} unreachable items with dangling ids"));
            }
            for problem in problems {
                // Without private items, an id can name a stripped private item: known, open
                // upstream (#113674, #119626, #117718, #112852). With them, nothing is stripped.
                if *name == "json" && problem.starts_with("id ") {
                    rec.notes.push(format!("json: {problem} (stripped item, #113674 family)"));
                } else {
                    rec.found.push(format!("json ({name}): {problem}"));
                }
            }
            if *name == "json" {
                public_json = Some(j);
            }
        }
    }
    if let Some(doc) = public_json {
        for m in reexport_missing(&doc, &root_reexports(&test.text)) {
            rec.found.push(format!("reexport: {m}"));
        }
        if !test.text.contains("no_core") && !test.text.contains("no_std") {
            let (code, probes) = auto_trait_probes(&doc, test.text.lines().count());
            rec.probes = probes.len();
            if !probes.is_empty()
                && let Some(src) = run_probes(tools, test, dir.path(), &code, &probes, &mut rec.found, &mut rec.notes)
            {
                files.push(("probe.rs".into(), src));
            }
        }
    }
    if !rec.found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &files, &serde_json::json!({ "found": rec.found, "notes": rec.notes, "rustdoc_flags": flags }));
    }
    rec
}

fn ice_line(stderr: &str) -> String {
    stderr
        .lines()
        .find(|l| l.contains("panicked at") || l.contains("internal compiler error"))
        .map(|l| l.chars().take(160).collect())
        .unwrap_or_default()
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let bin = PathBuf::from(std::env::var("HOME")?).join(format!(".rustup/toolchains/{}-x86_64-unknown-linux-gnu/bin", args.toolchain));
    let tools = Tools { rustc: bin.join("rustc"), rustdoc: bin.join("rustdoc") };
    anyhow::ensure!(tools.rustdoc.exists(), "no rustdoc at {}", tools.rustdoc.display());
    let tests = uitest::tests(&args.sweep.tests, uitest::ALL, |_| false);
    let tests = args.sweep.select(tests);
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, &tools, t)))
}

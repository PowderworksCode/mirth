//! Diagnostics coverage (docs/coverage-plan.md, rank 4): which of the compiler's error codes, lints,
//! diagnostic structs and suggestion applicabilities a corpus makes it emit, against all that exist.
//!
//! `--collect` compiles each standalone UI test with `--error-format=json` (the sweep driver:
//! resumable, `--jobs`) and keeps, per test, each diagnostic's level, code (an error code or a
//! lint name), message, child messages and labels, and suggestion applicabilities. Without it,
//! the report folds one or more such `results.jsonl` files against:
//!
//! - error codes: `rustc_error_codes/src/error_codes/E*.md` (those marked no longer emitted apart);
//! - lints: `rustc -W help` (by default level);
//! - diagnostic structs: every `#[derive(Diagnostic)]` and `#[derive(Subdiagnostic)]` item or
//!   variant with an inline message (`#[diag("…")]`, `#[note("…")]`, …), matched against the
//!   emitted messages with each `{$arg}` (and each `{$x -> …}` selector) as a wildcard. A template
//!   that is only an argument (`{$msg}`) matches anything and is left out as unmeasurable.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{self, Compile, Status};
use mirth_lab::sitemap::home;
use mirth_lab::uitest::{self, Test};
use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};
use syn::spanned::Spanned;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The compiler (for --collect, and for the lint list).
    #[arg(long, default_value_os_t = home("mirth-work/campaign/rustc/bin/rustc"))]
    rustc: PathBuf,
    /// Compile the corpus and save its diagnostics to <work>/results.jsonl.
    #[arg(long)]
    collect: bool,
    #[command(flatten)]
    sweep: Sweep,
    /// Collected results to report on (default: <work>/results.jsonl).
    #[arg(long)]
    data: Vec<PathBuf>,
    /// The rust checkout (error code docs, diagnostic structs).
    #[arg(long, default_value_os_t = home("mirth-work/rust"))]
    rust: PathBuf,
    /// Write the gap lists here.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Default)]
struct Diag {
    level: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    code: String,
    message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    applicability: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Rec {
    test: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    #[serde(default)]
    diags: Vec<Diag>,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        Vec::new()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

fn collect_one(args: &Args, test: &Test) -> Rec {
    let dir = driver::scratch_dir(&args.sweep);
    let c = Compile::new(&args.rustc, &test.path, dir.path(), &test.flags, test.edition()).emit("metadata").json().timeout(120).run();
    let mut rec = Rec { test: test.rel.clone(), skip: None, diags: Vec::new() };
    if matches!(c.status, Status::Timeout) {
        rec.skip = Some("timeout".into());
        return rec;
    }
    for d in rustc::diagnostics(&c.stderr) {
        let mut children = Vec::new();
        let mut appl = BTreeSet::new();
        for node in d.walk() {
            if !std::ptr::eq(node, &d) {
                children.push(node.message.clone());
            }
            for s in &node.spans {
                if let Some(l) = &s.label {
                    children.push(l.clone());
                }
                if s.suggested_replacement.is_some() {
                    appl.insert(s.suggestion_applicability.clone().unwrap_or_else(|| "Unspecified".into()));
                }
            }
        }
        children.sort();
        children.dedup();
        rec.diags.push(Diag { level: d.level.clone(), code: d.code().to_owned(), message: d.message.clone(), children, applicability: appl.into_iter().collect() });
    }
    rec
}

/// A message template as a regex: literal text, with `{$arg}` and `{$x -> …}` as wildcards.
fn template_regex(t: &str) -> Option<String> {
    let mut out = String::from("(?s)^");
    let mut literal = 0;
    let mut rest = t;
    while let Some(i) = rest.find('{') {
        out.push_str(&regex::escape(&rest[..i]));
        literal += rest[..i].trim().len();
        // Skip to the matching brace (selectors nest braces).
        let mut depth = 0;
        let mut end = rest.len();
        for (k, ch) in rest[i..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i + k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push_str(".*?");
        rest = &rest[end.min(rest.len())..];
    }
    literal += rest.trim().len();
    out.push_str(&regex::escape(rest));
    out.push('$');
    (literal >= 4).then_some(out)
}

struct Template {
    kind: &'static str,
    file: String,
    line: usize,
    name: String,
    text: String,
}

/// The inline messages of `#[derive(Diagnostic)]` and `#[derive(Subdiagnostic)]` items.
fn templates(rust: &Path) -> (Vec<Template>, usize) {
    const SUB: &[&str] = &["note", "help", "label", "warning", "suggestion", "multipart_suggestion", "suggestion_short", "suggestion_verbose", "suggestion_hidden"];
    let mut out = Vec::new();
    let mut slugs = 0;
    for (key, path) in mirth_lab::sitemap::compiler_files(rust) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if !text.contains("derive(Diagnostic") && !text.contains("derive(Subdiagnostic") {
            continue;
        }
        let Ok(file) = syn::parse_file(&text) else { continue };
        let mut items: Vec<(&[syn::Attribute], String, Vec<(&[syn::Attribute], String)>)> = Vec::new();
        fn walk<'a>(items: &'a [syn::Item], out: &mut Vec<(&'a [syn::Attribute], String, Vec<(&'a [syn::Attribute], String)>)>) {
            for it in items {
                match it {
                    syn::Item::Struct(s) => out.push((&s.attrs, s.ident.to_string(), vec![])),
                    syn::Item::Enum(e) => out.push((&e.attrs, e.ident.to_string(), e.variants.iter().map(|v| (&v.attrs[..], v.ident.to_string())).collect())),
                    syn::Item::Mod(m) => {
                        if let Some((_, inner)) = &m.content {
                            walk(inner, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        walk(&file.items, &mut items);
        for (attrs, name, variants) in items {
            let derive: String = attrs.iter().filter(|a| a.path().is_ident("derive")).map(|a| a.meta.to_token_stream_string()).collect();
            let kind = if derive.contains("Subdiagnostic") { "sub" } else if derive.contains("Diagnostic") { "diag" } else { continue };
            let wanted: &[&str] = if kind == "diag" { &["diag"] } else { SUB };
            let mut take = |attrs: &[syn::Attribute], name: String| {
                for a in attrs.iter().filter(|a| wanted.iter().any(|w| a.path().is_ident(w))) {
                    let first = a.parse_args_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated).ok().and_then(|p| p.into_iter().next());
                    match first {
                        Some(syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. })) => {
                            out.push(Template { kind, file: key.clone(), line: a.span().start().line, name: name.clone(), text: s.value() })
                        }
                        Some(_) => slugs += 1,
                        None => {}
                    }
                }
            };
            take(attrs, name.clone());
            for (vattrs, vname) in variants {
                take(vattrs, format!("{name}::{vname}"));
            }
        }
    }
    (out, slugs)
}

trait MetaString {
    fn to_token_stream_string(&self) -> String;
}
impl MetaString for syn::Meta {
    fn to_token_stream_string(&self) -> String {
        quote::ToTokens::to_token_stream(self).to_string()
    }
}

fn lints(rustc: &Path) -> BTreeMap<String, String> {
    static LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+([a-z0-9-]+)\s+(allow|warn|deny|forbid)\s").unwrap());
    let out = Command::new(rustc).args(["-W", "help"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut map = BTreeMap::new();
    for l in out.lines() {
        if l.contains("Lint groups provided") {
            break;
        }
        if let Some(c) = LINE.captures(l) {
            map.insert(c[1].replace('-', "_"), c[2].to_owned());
        }
    }
    map
}

fn error_codes(rust: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    let dir = rust.join("compiler/rustc_error_codes/src/error_codes");
    let (mut live, mut retired) = (BTreeSet::new(), BTreeSet::new());
    for f in mirth_lab::coverage::files_with(&dir, "md") {
        let code = f.file_stem().unwrap().to_string_lossy().into_owned();
        if std::fs::read_to_string(&f).unwrap_or_default().contains("no longer emitted by the compiler") {
            retired.insert(code);
        } else {
            live.insert(code);
        }
    }
    (live, retired)
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    if args.collect {
        let tests = args.sweep.select(uitest::tests(&args.sweep.tests, uitest::ALL, |_| false));
        println!("{} tests", tests.len());
        return Ok(driver::drive(&tests, &args.sweep, |t| collect_one(&args, t)));
    }
    let data = if args.data.is_empty() { vec![args.sweep.work.join("results.jsonl")] } else { args.data.clone() };
    let mut recs = 0;
    let mut codes: BTreeMap<String, usize> = BTreeMap::new();
    let mut appl: BTreeMap<String, usize> = BTreeMap::new();
    let mut machine_by_code: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut messages: BTreeMap<String, usize> = BTreeMap::new();
    let mut child_messages: BTreeMap<String, usize> = BTreeMap::new();
    let mut ices = 0;
    let mut lint_notes: BTreeSet<String> = BTreeSet::new();
    static LINT_NOTE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"#\[(?:warn|deny|forbid|expect)\(([\w:]+)\)\]|requested on the command line with `-[WDF] ([\w:-]+)`").unwrap());
    for path in &data {
        for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
            let Ok(r) = serde_json::from_str::<Rec>(line) else { continue };
            recs += 1;
            for d in &r.diags {
                if d.message.starts_with("internal compiler error") {
                    ices += 1;
                }
                if !d.code.is_empty() {
                    *codes.entry(d.code.clone()).or_default() += 1;
                }
                for a in &d.applicability {
                    *appl.entry(a.clone()).or_default() += 1;
                    machine_by_code.entry(if d.code.is_empty() { "-".into() } else { d.code.clone() }).or_default().insert(a.clone());
                }
                *messages.entry(d.message.clone()).or_default() += 1;
                // A lint with an error code of its own names the lint only in a note.
                for c in &d.children {
                    for m in LINT_NOTE.captures_iter(c) {
                        lint_notes.insert(m.get(1).or(m.get(2)).unwrap().as_str().replace('-', "_"));
                    }
                }
                for c in &d.children {
                    *child_messages.entry(c.clone()).or_default() += 1;
                }
            }
        }
    }
    anyhow::ensure!(recs > 0, "no collected diagnostics in {:?} (run with --collect first)", data);

    let (live, retired) = error_codes(&args.rust);
    let lint_list = lints(&args.rustc);
    let (tpls, slugs) = templates(&args.rust);
    let mut out = String::new();
    let _ = writeln!(out, "{recs} tests' diagnostics ({} distinct messages, {ices} ICE reports)", messages.len());

    let emitted_codes: BTreeSet<&String> = codes.keys().filter(|c| c.starts_with('E') && c[1..].chars().all(|x| x.is_ascii_digit())).collect();
    let live_hit = live.iter().filter(|c| emitted_codes.contains(c)).count();
    let retired_hit: Vec<&String> = retired.iter().filter(|c| emitted_codes.contains(c)).collect();
    let _ = writeln!(out, "\nerror codes: {} documented ({} marked no longer emitted); emitted {live_hit} of the {} live ones ({:.1}%); retired codes emitted: {:?}", live.len() + retired.len(), retired.len(), live.len(), 100.0 * live_hit as f64 / live.len().max(1) as f64, retired_hit);

    let mut by_level: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (lint, level) in &lint_list {
        let e = by_level.entry(level.as_str()).or_default();
        e.0 += 1;
        e.1 += (codes.contains_key(lint) || lint_notes.contains(lint)) as usize;
    }
    let lint_hit: usize = by_level.values().map(|v| v.1).sum();
    let _ = writeln!(out, "lints: {} in `rustc -W help`; emitted {lint_hit} ({:.1}%)", lint_list.len(), 100.0 * lint_hit as f64 / lint_list.len().max(1) as f64);
    for (level, (n, h)) in &by_level {
        let _ = writeln!(out, "  default {level:6} {h:4} of {n:4}");
    }
    let tools: BTreeSet<&String> = codes.keys().filter(|c| c.contains("::")).collect();
    if !tools.is_empty() {
        let _ = writeln!(out, "  tool lints seen (not in the denominator): {}", tools.len());
    }

    // Diagnostic structs.
    let measurable: Vec<(usize, String)> = tpls.iter().enumerate().filter_map(|(i, t)| template_regex(&t.text).map(|r| (i, r))).collect();
    let mut hit = vec![false; tpls.len()];
    for (kind, pool) in [("diag", &messages), ("sub", &child_messages)] {
        let idx: Vec<&(usize, String)> = measurable.iter().filter(|(i, _)| tpls[*i].kind == kind).collect();
        // RegexSet in chunks: one set of thousands of patterns compiles too slowly.
        for chunk in idx.chunks(400) {
            let set = RegexSet::new(chunk.iter().map(|(_, r)| r.as_str()))?;
            for m in pool.keys() {
                for k in set.matches(m).iter() {
                    hit[chunk[k].0] = true;
                }
            }
        }
    }
    let measurable_set: HashSet<usize> = measurable.iter().map(|(i, _)| *i).collect();
    for kind in ["diag", "sub"] {
        let all = tpls.iter().filter(|t| t.kind == kind).count();
        let m = (0..tpls.len()).filter(|i| tpls[*i].kind == kind && measurable_set.contains(i)).count();
        let h = (0..tpls.len()).filter(|i| tpls[*i].kind == kind && hit[*i]).count();
        let what = if kind == "diag" { "diagnostic messages (#[diag])" } else { "subdiagnostic messages (#[note]/#[help]/#[label]/#[suggestion] on Subdiagnostic items)" };
        let _ = writeln!(out, "{what}: {all}; {m} measurable (not just an argument); emitted {h} ({:.1}%)", 100.0 * h as f64 / m.max(1) as f64);
    }
    if slugs > 0 {
        let _ = writeln!(out, "  {slugs} messages given by slug, not inline: not matched");
    }

    let _ = writeln!(out, "\nsuggestion applicabilities emitted:");
    for (a, n) in &appl {
        let _ = writeln!(out, "  {a:20} {n}");
    }
    let lints_with_machine = lint_list.keys().filter(|l| machine_by_code.get(*l).is_some_and(|s| s.contains("MachineApplicable"))).count();
    let _ = writeln!(out, "  lints with a machine-applicable suggestion seen: {lints_with_machine}");
    print!("{out}");

    if let Some(dir) = &args.out {
        std::fs::create_dir_all(dir)?;
        let mut text = format!("# Error codes never emitted: {}\n\n", live.len() - live_hit);
        for c in live.iter().filter(|c| !emitted_codes.contains(c)) {
            let _ = writeln!(text, "- {c}");
        }
        let _ = writeln!(text, "\n# Lints never emitted: {}\n", lint_list.len() - lint_hit);
        for (l, level) in lint_list.iter().filter(|(l, _)| !codes.contains_key(*l) && !lint_notes.contains(*l)) {
            let _ = writeln!(text, "- {l} ({level})");
        }
        std::fs::write(dir.join("gaps-diagnostics.md"), text)?;
        let mut by_file: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for (i, t) in tpls.iter().enumerate() {
            if measurable_set.contains(&i) && !hit[i] {
                let krate = t.file.split('/').nth(1).unwrap_or("").to_owned();
                by_file.entry((krate, t.file.clone())).or_default().push(format!("- {} {} `{}`: {:?}", t.line, t.kind, t.name, t.text.chars().take(100).collect::<String>()));
            }
        }
        let total: usize = by_file.values().map(Vec::len).sum();
        let mut text = format!("# Diagnostic messages never emitted: {total}\n\n");
        let mut current = String::new();
        for ((k, f), v) in &by_file {
            if *k != current {
                let n: usize = by_file.iter().filter(|((kk, _), _)| kk == k).map(|(_, v)| v.len()).sum();
                let _ = writeln!(text, "## {k} ({n})\n");
                current = k.clone();
            }
            let _ = writeln!(text, "### {f} ({})\n\n{}\n", v.len(), v.join("\n"));
        }
        std::fs::write(dir.join("gaps-diag-structs.md"), text)?;
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::template_regex;

    #[test]
    fn templates_match_messages() {
        let r = regex::Regex::new(&template_regex("cannot move a value of type `{$ty}`").unwrap()).unwrap();
        assert!(r.is_match("cannot move a value of type `[u8]`"));
        assert!(!r.is_match("cannot move a value of type `[u8]` here"));
        let r = regex::Regex::new(&template_regex("{$n -> [one] a field *[other] fields} never read").unwrap()).unwrap();
        assert!(r.is_match("fields `a` and `b` never read"));
        assert!(template_regex("{$msg}").is_none());
    }
}

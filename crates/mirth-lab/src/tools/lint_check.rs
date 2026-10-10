//! Lint oracles: a lint fires only when its premise holds, and acting on it (allowing it, or
//! removing what it flags) keeps the program compiling with the same behavior.
//!
//! For each standalone UI test that compiles without errors (lints capped to warnings where the
//! test denies them), with a column of allow-by-default lints turned on (`WIDEN`), and for each
//! lint that warns:
//!
//! - allow: `#![allow(<lint>)]` added to the crate removes that lint's warnings and changes no
//!   other diagnostic (a lint with side effects on others, or an allow that errors)
//! - delete: what a premise lint flags, removed independently of its suggestion, still compiles,
//!   and a run-pass test still prints the same: every item `dead_code` calls unused at once (with
//!   impls naming a dead type or trait, impl items of dead trait items, and unused imports of
//!   them), each arm `unreachable_patterns` flags, the statements from an `unreachable_code`
//!   statement up to the block's tail, each `unused_extern_crates` item
//! - premise: `trivial_numeric_casts` says the expression already has the type, so the cast
//!   goes; `trivial_casts` says a coercion would do, so `let p = e as T;` becomes
//!   `let c = e; let p: T = c;`; `ambiguous_wide_pointer_comparisons` says the operands are wide,
//!   so the left operand goes through a const assertion that it is two words, peeling up to three
//!   references (#163840); `missing_copy_implementations` says the type could be `Copy`, so
//!   `impl Copy` (and `Clone` if needed) is added after it
//! - widen-fix: a machine-applicable suggestion of an allow-by-default lint, applied alone,
//!   compiles and fixes the warning (suggest-diff never sees these lints); when it breaks alone
//!   but the same alternative of all of that lint's warnings applied together compiles (what
//!   `cargo fix` does), it is not a finding
//!
//! Edits are found with syn on the test file (byte ranges of the flagged spans); flagged code
//! inside macro definitions or invocations matches no syntax node and is left alone. Failures
//! the lint's design or the edit explains (`EXPLAINED`, exempt dead code, re-exports,
//! macro-generated users) are listed under `expected` instead of counted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{self, Compile, Diagnostic, Observed, Status, observe};
use mirth_lab::uitest::{self, Kind, Test};
use mirth_lab::normalize;
use regex::Regex;
use serde::Serialize;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// Leave the allow-by-default lints off.
    #[arg(long)]
    no_widen: bool,
    /// Lints tried with `allow` per test.
    #[arg(long, default_value_t = 6)]
    max_allow: usize,
    /// Suggestions of allow-by-default lints tried per test.
    #[arg(long, default_value_t = 4)]
    max_fix: usize,
    #[command(flatten)]
    sweep: Sweep,
}

/// Allow-by-default lints with an oracle here: a premise, a deletion or a suggestion to apply.
const WIDEN: &[&str] = &[
    "trivial_casts",
    "trivial_numeric_casts",
    "unused_qualifications",
    "unused_import_braces",
    "unused_lifetimes",
    "redundant_lifetimes",
    "single_use_lifetimes",
    "explicit_outlives_requirements",
    "redundant_imports",
    "elided_lifetimes_in_paths",
    "unreachable_pub",
    "unused_extern_crates",
    "missing_copy_implementations",
    "let_underscore_drop",
    "unit_bindings",
    "ambiguous_negative_literals",
];

/// A failed edit explained without a finding: known upstream, or what the edit cannot express.
/// (lint, what the test's source contains, new error codes all among these, explanation)
const EXPLAINED: &[(&str, &str, &[&str], &str)] = &[
    ("dead_code", "inherent_associated_types", &[], "known: rust-lang/rust#110332 (inherent associated types always dead)"),
    // Every impl in a staged_api crate needs a stability attribute, the added `impl Copy` too.
    ("missing_copy_implementations", "staged_api", &[], "edit: staged_api wants stability attributes"),
    // Unreachable statements still take part in type inference.
    ("unreachable_code", "", &["E0282", "E0283", "E0284"], "expected: unreachable code takes part in inference"),
    ("unreachable_patterns", "", &["E0282", "E0283", "E0284"], "expected: unreachable arms take part in inference"),
    // `match x {}` reads `x`; an arm with a wildcard pattern did not.
    ("unreachable_patterns", "", &["E0381"], "expected: without its last arm, a match reads its scrutinee"),
];

static BY_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*;|include(_str|_bytes)?!|#\[path").unwrap());
static SUMMARY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+ warnings? emitted|aborting due to)").unwrap());
static CLOSURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{(closure|coroutine|async block|async closure)@[^}]*\}").unwrap());
static BACKTICKED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`]*`").unwrap());
static NO_CORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#!\[no_core\]").unwrap());

#[derive(Serialize)]
struct Rec {
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    lints: Vec<String>,
    compiles: usize,
    found: Vec<String>,
    /// Differences explained without a finding.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    expected: Vec<String>,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        self.found.clone()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

/// A diagnostic as compared across edits: level, lint or code, message, primary line.
type Key = (String, String, String, usize);

/// `line_shift` lines were added at the top of `file`: its line numbers, in spans and in
/// messages (`{closure@file.rs:7:13}`), are moved back.
fn keys(diags: &[Diagnostic], line_shift: usize, file: &str) -> BTreeMap<Key, usize> {
    let in_message = Regex::new(&format!(r"{}:(\d+):", regex::escape(file))).unwrap();
    let mut out = BTreeMap::new();
    for d in diags {
        if SUMMARY.is_match(&d.message) {
            continue;
        }
        let line = d.primary().map_or(0, |s| {
            let ours = Path::new(&s.file_name).file_name().and_then(|n| n.to_str()) == Some(file);
            if ours { s.line_start.saturating_sub(line_shift) } else { s.line_start }
        });
        let message = in_message
            .replace_all(&d.message, |c: &regex::Captures| format!("{file}:{}:", c[1].parse::<usize>().unwrap_or(0).saturating_sub(line_shift)))
            .into_owned();
        // Closures print as `{closure@<path>:L:C: L:C}`, the path absolute or not.
        let message = CLOSURE.replace_all(&message, "{closure}").into_owned();
        *out.entry((d.level.clone(), d.code().to_owned(), message, line)).or_insert(0) += 1;
    }
    out
}

/// Lint names are snake_case words; error codes are `E` and digits.
fn is_lint(code: &str) -> bool {
    !code.is_empty() && !(code.starts_with('E') && code[1..].chars().all(|c| c.is_ascii_digit()))
}

fn errors(diags: &[Diagnostic]) -> BTreeSet<String> {
    diags
        .iter()
        .filter(|d| d.level == "error" && !SUMMARY.is_match(&d.message))
        .map(|d| if d.code().is_empty() { d.message.clone() } else { d.code().to_owned() })
        .collect()
}

// ---- finding the flagged syntax ----

type Range = std::ops::Range<usize>;

fn range(s: &impl Spanned) -> Range {
    s.span().byte_range()
}

/// The syntax the edits need, by byte range.
#[derive(Default)]
struct Index {
    /// Items, impl items and trait items: (range of the whole item, range of its name).
    items: Vec<(Range, Range, ItemKind)>,
    /// Inherent and trait impls: (range, self type name, item names, is a trait impl).
    impls: Vec<(Range, String, Vec<Range>)>,
    /// Trait impls: (range, trait name, self type name).
    trait_impls: Vec<(Range, String, String)>,
    /// `let` statements without a type: (statement range, pattern range, initializer range).
    lets: Vec<(Range, Range, Range)>,
    /// Items dead code may use without being reported: `#[allow(dead_code)]` or `_` names.
    exempt: Vec<Range>,
    /// Types with `#[allow(dead_code)]`: their impls are exempt too.
    exempt_types: Vec<String>,
    /// Macro definitions and invocations: code syn does not see.
    macros: Vec<Range>,
    /// Match arms: (arm range, pattern range, or-pattern alternatives).
    arms: Vec<(Range, Range, Vec<Range>)>,
    /// Blocks: the ranges of their statements, and whether the last is a tail expression.
    blocks: Vec<(Vec<Range>, bool)>,
    /// Casts: (range, expression range, type range).
    casts: Vec<(Range, Range, Range)>,
    /// Comparisons: (range, left operand range).
    comparisons: Vec<(Range, Range)>,
    /// Trait items: (name range, trait, item name).
    trait_items: Vec<(Range, String, String)>,
    /// Items of trait impls: (range, trait, item name).
    trait_impl_items: Vec<(Range, String, String)>,
    /// Every impl: (range, the words of its self type, the words of its trait and bounds).
    all_impls: Vec<(Range, BTreeSet<String>, BTreeSet<String>)>,
    /// Inline modules.
    mods: Vec<Range>,
    /// `use` items: (range, range of the tree).
    uses: Vec<(Range, Range)>,
    /// Non-generic structs and enums: (range, name, derives Clone).
    plain_types: Vec<(Range, String, bool)>,
}

#[derive(Clone, Copy, PartialEq)]
enum ItemKind {
    Type,
    Trait,
    Other,
}

impl<'a> Visit<'a> for Index {
    fn visit_item(&mut self, i: &'a syn::Item) {
        let name = match i {
            syn::Item::Fn(f) => Some((range(&f.sig.ident), ItemKind::Other)),
            syn::Item::Struct(s) => Some((range(&s.ident), ItemKind::Type)),
            syn::Item::Enum(e) => Some((range(&e.ident), ItemKind::Type)),
            syn::Item::Union(u) => Some((range(&u.ident), ItemKind::Type)),
            syn::Item::Const(c) => Some((range(&c.ident), ItemKind::Other)),
            syn::Item::Static(s) => Some((range(&s.ident), ItemKind::Other)),
            syn::Item::Type(t) => Some((range(&t.ident), ItemKind::Type)),
            syn::Item::Trait(t) => Some((range(&t.ident), ItemKind::Trait)),
            syn::Item::TraitAlias(t) => Some((range(&t.ident), ItemKind::Trait)),
            syn::Item::Mod(m) => Some((range(&m.ident), ItemKind::Other)),
            syn::Item::ExternCrate(e) => Some((range(e), ItemKind::Other)),
            syn::Item::Macro(m) => m.ident.as_ref().map(|id| (range(id), ItemKind::Other)),
            _ => None,
        };
        if let Some((n, k)) = name {
            self.items.push((range(i), n, k));
        }
        let attrs: &[syn::Attribute] = match i {
            syn::Item::Fn(f) => &f.attrs,
            syn::Item::Struct(s) => &s.attrs,
            syn::Item::Enum(e) => &e.attrs,
            syn::Item::Impl(m) => &m.attrs,
            syn::Item::Trait(t) => &t.attrs,
            syn::Item::Const(c) => &c.attrs,
            syn::Item::Static(s) => &s.attrs,
            syn::Item::Mod(m) => &m.attrs,
            _ => &[],
        };
        if let syn::Item::Mod(m) = i
            && m.content.is_some()
        {
            self.mods.push(range(i));
        }
        let underscore = match i {
            syn::Item::Type(t) => t.ident.to_string().starts_with('_'),
            syn::Item::Fn(f) => f.sig.ident.to_string().starts_with('_'),
            syn::Item::Const(c) => c.ident.to_string().starts_with('_'),
            syn::Item::Static(s) => s.ident.to_string().starts_with('_'),
            syn::Item::Struct(s) => s.ident.to_string().starts_with('_'),
            _ => false,
        };
        // Trait aliases: dead_code does not report them, so what they use is not dead to it.
        if underscore || allows_dead(attrs) || matches!(i, syn::Item::TraitAlias(_)) {
            self.exempt.push(range(i));
            if let syn::Item::Struct(s) = i {
                self.exempt_types.push(s.ident.to_string());
            }
            if let syn::Item::Enum(e) = i {
                self.exempt_types.push(e.ident.to_string());
            }
        }
        if let syn::Item::Trait(t) = i {
            for it in &t.items {
                let id = match it {
                    syn::TraitItem::Fn(f) => &f.sig.ident,
                    syn::TraitItem::Const(c) => &c.ident,
                    syn::TraitItem::Type(ty) => &ty.ident,
                    _ => continue,
                };
                self.trait_items.push((range(id), t.ident.to_string(), id.to_string()));
            }
        }
        match i {
            syn::Item::Struct(s) if s.generics.params.is_empty() => self.plain_types.push((range(i), s.ident.to_string(), derives(&s.attrs, "Clone"))),
            syn::Item::Enum(e) if e.generics.params.is_empty() => self.plain_types.push((range(i), e.ident.to_string(), derives(&e.attrs, "Clone"))),
            syn::Item::Use(u) => self.uses.push((range(i), range(&u.tree))),
            syn::Item::Impl(imp) => {
                fn words_of(ts: proc_macro2::TokenStream) -> BTreeSet<String> {
                    let mut words = BTreeSet::new();
                    let mut collect = |ts: proc_macro2::TokenStream| {
                    fn walk(ts: proc_macro2::TokenStream, out: &mut BTreeSet<String>) {
                        for t in ts {
                            match t {
                                proc_macro2::TokenTree::Ident(i) => {
                                    out.insert(i.to_string());
                                }
                                proc_macro2::TokenTree::Group(g) => walk(g.stream(), out),
                                _ => {}
                            }
                        }
                    }
                    walk(ts, &mut words);
                    };
                    collect(ts);
                    words
                }
                let self_words = words_of(quote::ToTokens::to_token_stream(&*imp.self_ty));
                let mut other = words_of(quote::ToTokens::to_token_stream(&imp.generics));
                if let Some(w) = &imp.generics.where_clause {
                    other.extend(words_of(quote::ToTokens::to_token_stream(w)));
                }
                if let Some((_, path, _)) = &imp.trait_ {
                    other.extend(words_of(quote::ToTokens::to_token_stream(path)));
                }
                self.all_impls.push((range(i), self_words, other));
                if let syn::Type::Path(p) = &*imp.self_ty
                    && let Some(seg) = p.path.segments.last()
                {
                    let names = imp
                        .items
                        .iter()
                        .filter_map(|it| match it {
                            syn::ImplItem::Fn(f) => Some(range(&f.sig.ident)),
                            syn::ImplItem::Const(c) => Some(range(&c.ident)),
                            syn::ImplItem::Type(t) => Some(range(&t.ident)),
                            _ => None,
                        })
                        .collect();
                    if imp.trait_.is_none() {
                        self.impls.push((range(i), seg.ident.to_string(), names));
                    } else {
                        self.impls.push((range(i), seg.ident.to_string(), Vec::new()));
                    }
                }
                if let Some((_, path, _)) = &imp.trait_
                    && let Some(seg) = path.segments.last()
                {
                    let self_name = match &*imp.self_ty {
                        syn::Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default(),
                        _ => String::new(),
                    };
                    self.trait_impls.push((range(i), seg.ident.to_string(), self_name));
                    for it in &imp.items {
                        let name = match it {
                            syn::ImplItem::Fn(f) => f.sig.ident.to_string(),
                            syn::ImplItem::Const(c) => c.ident.to_string(),
                            syn::ImplItem::Type(t) => t.ident.to_string(),
                            _ => continue,
                        };
                        self.trait_impl_items.push((range(it), seg.ident.to_string(), name));
                    }
                }
            }
            _ => {}
        }
        visit::visit_item(self, i);
    }
    fn visit_impl_item(&mut self, i: &'a syn::ImplItem) {
        let name = match i {
            syn::ImplItem::Fn(f) => Some(range(&f.sig.ident)),
            syn::ImplItem::Const(c) => Some(range(&c.ident)),
            syn::ImplItem::Type(t) => Some(range(&t.ident)),
            _ => None,
        };
        if let Some(n) = name {
            self.items.push((range(i), n, ItemKind::Other));
        }
        visit::visit_impl_item(self, i);
    }
    fn visit_trait_item(&mut self, i: &'a syn::TraitItem) {
        let name = match i {
            syn::TraitItem::Fn(f) => Some(range(&f.sig.ident)),
            syn::TraitItem::Const(c) => Some(range(&c.ident)),
            syn::TraitItem::Type(t) => Some(range(&t.ident)),
            _ => None,
        };
        if let Some(n) = name {
            self.items.push((range(i), n, ItemKind::Other));
        }
        visit::visit_trait_item(self, i);
    }
    fn visit_foreign_item(&mut self, i: &'a syn::ForeignItem) {
        let name = match i {
            syn::ForeignItem::Fn(f) => Some(range(&f.sig.ident)),
            syn::ForeignItem::Static(s) => Some(range(&s.ident)),
            syn::ForeignItem::Type(t) => Some(range(&t.ident)),
            _ => None,
        };
        if let Some(n) = name {
            self.items.push((range(i), n, ItemKind::Other));
        }
        visit::visit_foreign_item(self, i);
    }
    fn visit_macro(&mut self, m: &'a syn::Macro) {
        self.macros.push(range(m));
        visit::visit_macro(self, m);
    }
    fn visit_arm(&mut self, a: &'a syn::Arm) {
        let alts = match &a.pat {
            syn::Pat::Or(o) => o.cases.iter().map(range).collect(),
            _ => Vec::new(),
        };
        self.arms.push((range(a), range(&a.pat), alts));
        visit::visit_arm(self, a);
    }
    fn visit_local(&mut self, l: &'a syn::Local) {
        if !matches!(l.pat, syn::Pat::Type(_))
            && let Some(init) = &l.init
        {
            self.lets.push((range(l), range(&l.pat), range(&*init.expr)));
        }
        visit::visit_local(self, l);
    }
    fn visit_trait_item_fn(&mut self, f: &'a syn::TraitItemFn) {
        if f.sig.ident.to_string().starts_with('_') || allows_dead(&f.attrs) {
            self.exempt.push(range(f));
        }
        visit::visit_trait_item_fn(self, f);
    }
    fn visit_impl_item_fn(&mut self, f: &'a syn::ImplItemFn) {
        if f.sig.ident.to_string().starts_with('_') || allows_dead(&f.attrs) {
            self.exempt.push(range(f));
        }
        visit::visit_impl_item_fn(self, f);
    }
    fn visit_block(&mut self, b: &'a syn::Block) {
        self.blocks.push((b.stmts.iter().map(range).collect(), matches!(b.stmts.last(), Some(syn::Stmt::Expr(_, None)))));
        visit::visit_block(self, b);
    }
    fn visit_expr_cast(&mut self, c: &'a syn::ExprCast) {
        self.casts.push((range(c), range(&*c.expr), range(&*c.ty)));
        visit::visit_expr_cast(self, c);
    }
    fn visit_expr_binary(&mut self, b: &'a syn::ExprBinary) {
        if matches!(b.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_) | syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_)) {
            self.comparisons.push((range(b), range(&*b.left)));
        }
        visit::visit_expr_binary(self, b);
    }
}

fn derives(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("derive")
            && a.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
                .is_ok_and(|ps| ps.iter().any(|p| p.segments.last().is_some_and(|s| s.ident == name)))
    })
}

fn allows_dead(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        (a.path().is_ident("allow") || a.path().is_ident("expect"))
            && a.meta.require_list().is_ok_and(|l| {
                let t = l.tokens.to_string();
                t.contains("dead_code") || t.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "unused")
            })
    })
}

fn index(text: &str) -> Option<Index> {
    let file = syn::parse_file(text).ok()?;
    let mut ix = Index::default();
    ix.visit_file(&file);
    Some(ix)
}

/// Replacements (range, new text), applied together from the end; overlapping ones are dropped
/// in favor of the enclosing one.
fn apply(text: &str, mut edits: Vec<(Range, String)>) -> Option<String> {
    edits.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));
    let mut kept: Vec<(Range, String)> = Vec::new();
    for e in edits {
        if kept.last().is_some_and(|k| e.0.start < k.0.end) {
            continue;
        }
        kept.push(e);
    }
    if kept.is_empty() {
        return None;
    }
    let mut out = text.to_owned();
    for (r, s) in kept.iter().rev() {
        if r.end > out.len() || !out.is_char_boundary(r.start) || !out.is_char_boundary(r.end) {
            return None;
        }
        out.replace_range(r.clone(), s);
    }
    Some(out)
}

fn spans(d: &Diagnostic, file: &str) -> Vec<Range> {
    d.spans
        .iter()
        .filter(|s| s.is_primary && Path::new(&s.file_name).file_name().and_then(|n| n.to_str()) == Some(file))
        .map(|s| s.byte_start..s.byte_end)
        .collect()
}

/// An edit to try: what it tests, and the new text.
struct Edit {
    what: String,
    lint: String,
    text: String,
    /// The lint's warnings must go away (a fix); otherwise only "still compiles, same output".
    fixes: Option<(String, usize)>,
    /// Compile to an object file (a const assertion needs monomorphization).
    mono: bool,
    /// A wide-pointer premise: the left operand (range, text), checked through 0-3 derefs.
    wide: Option<(Range, String)>,
    /// For a fix: every fix of the same lint applied together (what `cargo fix` does).
    together: Option<String>,
}

const WIDE_HELPER: &str = "\n#[allow(dead_code)]\nfn __mirth_wide<P>(p: &P) -> &P {\n    const { assert!(size_of::<P>() == 2 * size_of::<usize>(), \"mirth: not a wide pointer\") };\n    p\n}\n";

fn edits(args: &Args, text: &str, file: &str, ix: &Index, diags: &[Diagnostic], no_core: bool) -> Vec<Edit> {
    let mut out = Vec::new();
    fn warns<'d>(diags: &'d [Diagnostic], lint: &'static str) -> impl Iterator<Item = &'d Diagnostic> {
        diags.iter().filter(move |d| d.level == "warning" && d.code() == lint)
    }

    // dead_code: every unused item at once, with the impls of dead types and traits.
    let dead: Vec<Range> = warns(diags, "dead_code")
        .chain(warns(diags, "dead_code_pub_in_binary"))
        .filter(|d| {
            // Fields and variants stay (constructors and patterns name them); names in
            // backticks are not the kind of item.
            let kind = BACKTICKED.replace_all(&d.message, "");
            !(kind.contains("field") || kind.contains("variant") || kind.contains("never read"))
        })
        .flat_map(|d| spans(d, file))
        .collect();
    if !dead.is_empty() {
        let mut del: Vec<(Range, String)> = Vec::new();
        // (name, the inline module it is in): impls match by name within the same module.
        let mut dead_names: BTreeSet<(String, Option<(usize, usize)>)> = BTreeSet::new();
        let mut dead_traits: BTreeSet<(String, Option<(usize, usize)>)> = BTreeSet::new();
        let module = |r: &Range| ix.mods.iter().filter(|m| m.start <= r.start && r.end <= m.end && **m != *r).min_by_key(|m| m.end - m.start).map(|m| (m.start, m.end));
        // All or nothing: a dead item the index lacks (in a macro, or syntax syn does not
        // know) may be what another dead item uses.
        let all_found = dead.iter().all(|sp| ix.items.iter().any(|(_, n, _)| n == sp));
        for (whole, name, kind) in ix.items.iter().filter(|_| all_found) {
            if dead.contains(name) {
                let ident = text.get(name.clone()).unwrap_or("").to_owned();
                match kind {
                    ItemKind::Type => {
                        // Only when every inherent item of the type is dead too.
                        let all_dead = ix.impls.iter().filter(|(_, t, _)| *t == ident).all(|(_, _, items)| items.iter().all(|r| dead.contains(r)));
                        if !all_dead {
                            continue;
                        }
                        dead_names.insert((ident, module(whole)));
                    }
                    ItemKind::Trait => {
                        dead_traits.insert((ident, module(whole)));
                    }
                    ItemKind::Other => {}
                }
                del.push((whole.clone(), String::new()));
            }
        }
        // Imports of what is deleted are unused too.
        for sp in warns(diags, "unused_imports").flat_map(|d| spans(d, file)) {
            let single = |t: &Range| text.get(t.clone()).is_some_and(|s| !s.contains(',')) && t.start <= sp.start && sp.end <= t.end;
            if let Some((r, _)) = ix.uses.iter().find(|(r, t)| *t == sp || *r == sp || single(t)) {
                del.push((r.clone(), String::new()));
            }
        }
        // A dead trait item goes with its definitions in the trait's impls.
        for (n, tr, item) in &ix.trait_items {
            if all_found && dead.contains(n) {
                for (r, t2, i2) in &ix.trait_impl_items {
                    if t2 == tr && i2 == item {
                        del.push((r.clone(), String::new()));
                    }
                }
            }
        }
        // Impls naming a deleted type or trait anywhere in their header go with it.
        for (r, self_words, other) in &ix.all_impls {
            let m = module(r);
            let named = |set: &BTreeSet<(String, Option<(usize, usize)>)>, w: &String| set.contains(&(w.clone(), m.clone()));
            if self_words.iter().any(|w| named(&dead_names, w) || named(&dead_traits, w)) || other.iter().any(|w| named(&dead_traits, w)) {
                del.push((r.clone(), String::new()));
            }
        }
        if let Some(t) = apply(text, del).filter(|_| all_found) {
            out.push(Edit { what: "delete".into(), lint: "dead_code".into(), text: t, fixes: None, mono: false, wide: None, together: None });
        }
    }

    // unreachable_patterns: each flagged arm (or alternative) alone.
    for d in warns(diags, "unreachable_patterns").filter(|d| d.message.starts_with("unreachable pattern")) {
        for sp in spans(d, file) {
            let edit = ix.arms.iter().find_map(|(arm, pat, alts)| {
                if *pat == sp {
                    return Some((arm.clone(), String::new()));
                }
                let i = alts.iter().position(|a| *a == sp)?;
                Some(if i + 1 < alts.len() { (alts[i].start..alts[i + 1].start, String::new()) } else { (alts[i - 1].end..alts[i].end, String::new()) })
            });
            if let Some(t) = edit.and_then(|e| apply(text, vec![e])) {
                out.push(Edit { what: "delete".into(), lint: "unreachable_patterns".into(), text: t, fixes: None, mono: false, wide: None, together: None });
            }
        }
    }

    // unreachable_code: from the flagged statement to the end of its block.
    for d in warns(diags, "unreachable_code").filter(|d| d.message.starts_with("unreachable statement") || d.message.starts_with("unreachable expression")) {
        // The code that diverges (the secondary span) runs; inside the flagged code, it goes too.
        let causes: Vec<Range> = d.spans.iter().filter(|s| !s.is_primary).map(|s| s.byte_start..s.byte_end).collect();
        for sp in spans(d, file) {
            if causes.iter().any(|c| sp.start <= c.start && c.end <= sp.end) {
                continue;
            }
            // A tail expression stays: unreachable code still gives the block its type.
            let edit = ix.blocks.iter().find_map(|(stmts, tail)| {
                let i = stmts.iter().position(|s| s.start == sp.start && (s.end == sp.end || s.end == sp.end + 1))?;
                let last = if *tail { stmts.len().checked_sub(2)? } else { stmts.len() - 1 };
                (i <= last).then(|| (stmts[i].start..stmts[last].end, String::new()))
            });
            if let Some(t) = edit.and_then(|e| apply(text, vec![e])) {
                out.push(Edit { what: "delete".into(), lint: "unreachable_code".into(), text: t, fixes: None, mono: false, wide: None, together: None });
            }
        }
    }

    // unused_extern_crates: delete the item.
    let unused_crates: Vec<(Range, String)> = warns(diags, "unused_extern_crates")
        .flat_map(|d| spans(d, file))
        .filter_map(|sp| ix.items.iter().find(|(w, n, _)| *w == sp || *n == sp).map(|(w, _, _)| (w.clone(), String::new())))
        .collect();
    if let Some(t) = apply(text, unused_crates) {
        out.push(Edit { what: "delete".into(), lint: "unused_extern_crates".into(), text: t, fixes: None, mono: false, wide: None, together: None });
    }

    // trivial_numeric_casts: the expression already has the type, so the cast goes.
    let casts: Vec<(Range, String)> = warns(diags, "trivial_numeric_casts")
        .flat_map(|d| spans(d, file))
        .filter_map(|sp| ix.casts.iter().find(|(c, _, _)| *c == sp))
        .filter_map(|(c, e, _)| Some((c.clone(), format!("({})", text.get(e.clone())?))))
        .collect();
    if let Some(t) = apply(text, casts) {
        out.push(Edit { what: "premise".into(), lint: "trivial_numeric_casts".into(), text: t, fixes: None, mono: false, wide: None, together: None });
    }
    // trivial_casts: a coercion would do. Where the cast initializes an untyped `let`, the
    // value goes into a `let` of its own and then through the annotated binding: a coercion
    // site, with the same temporary lifetimes and no expected type flowing into the value.
    let casts: Vec<(Range, String)> = warns(diags, "trivial_casts")
        .flat_map(|d| spans(d, file))
        .filter_map(|sp| ix.casts.iter().find(|(c, _, _)| *c == sp))
        .filter_map(|(c, e, t)| {
            let (stmt, pat, _) = ix.lets.iter().find(|(_, _, init)| init == c)?;
            // A pattern with a `ref` binding makes the initializer an equality, not a coercion.
            if text.get(pat.clone())?.split(|ch: char| !ch.is_alphanumeric() && ch != '_').any(|w| w == "ref") {
                return None;
            }
            Some([
                (stmt.start..stmt.start, format!("let __mirth_cast = {}; ", text.get(e.clone())?)),
                (pat.end..pat.end, format!(": {}", text.get(t.clone())?)),
                (c.clone(), "__mirth_cast".to_owned()),
            ])
        })
        .flatten()
        .collect();
    if let Some(t) = apply(text, casts) {
        out.push(Edit { what: "premise".into(), lint: "trivial_casts".into(), text: t, fixes: None, mono: false, wide: None, together: None });
    }

    // Wide pointer comparisons: each left operand is two words, or a reference to one that is.
    if !no_core {
        for (l, _) in warns(diags, "ambiguous_wide_pointer_comparisons")
            .flat_map(|d| spans(d, file))
            .filter_map(|sp| ix.comparisons.iter().find(|(c, _)| *c == sp).map(|(_, l)| (l.clone(), ())))
            .take(6)
        {
            if let Some(operand) = text.get(l.clone()) {
                out.push(Edit { what: "premise".into(), lint: "ambiguous_wide_pointer_comparisons".into(), text: String::new(), fixes: None, mono: true, wide: Some((l, operand.to_owned())), together: None });
            }
        }
    }

    // missing_copy_implementations: the type can be Copy.
    if !no_core {
        for d in warns(diags, "missing_copy_implementations") {
            for sp in spans(d, file) {
                if let Some((r, name, derived)) = ix.plain_types.iter().find(|(r, _, _)| *r == sp || (r.start <= sp.start && sp.end <= r.end)) {
                    let has_clone = *derived || ix.trait_impls.iter().any(|(_, t, s)| t == "Clone" && s == name);
                    let mut add = format!("\nimpl Copy for {name} {{}}\n");
                    if !has_clone {
                        add += &format!("impl Clone for {name} {{ fn clone(&self) -> Self {{ *self }} }}\n");
                    }
                    if let Some(t) = apply(text, vec![(r.end..r.end, add)]) {
                        out.push(Edit { what: "premise".into(), lint: "missing_copy_implementations".into(), text: t, fixes: None, mono: false, wide: None, together: None });
                    }
                }
            }
        }
    }

    // The suggestions of allow-by-default lints, one at a time.
    let suggestion = |node: &Diagnostic| -> Vec<(Range, String)> {
        node.spans
            .iter()
            .filter(|s| s.suggestion_applicability.as_deref() == Some("MachineApplicable") && Path::new(&s.file_name).file_name().and_then(|n| n.to_str()) == Some(file))
            .filter_map(|s| Some((s.byte_start..s.byte_end, s.suggested_replacement.clone()?)))
            .collect()
    };
    // The first suggestion of each warning of a lint, together.
    // The `alt`-th suggestion of every warning of a lint, together (alternatives stay apart).
    let together = |lint: &str, alt: usize| -> Option<String> {
        let all: Vec<(Range, String)> = diags
            .iter()
            .filter(|d| d.level == "warning" && d.code() == lint)
            .filter_map(|d| std::iter::once(d).chain(&d.children).map(&suggestion).filter(|p| !p.is_empty()).nth(alt))
            .flatten()
            .collect();
        apply(text, all)
    };
    let mut fixes = 0;
    for d in diags.iter().filter(|d| d.level == "warning" && WIDEN.contains(&d.code())) {
        let mut alt = 0;
        for node in std::iter::once(d).chain(&d.children) {
            if fixes >= args.max_fix {
                break;
            }
            let parts = suggestion(node);
            if parts.is_empty() {
                continue;
            }
            alt += 1;
            if let Some(t) = apply(text, parts) {
                fixes += 1;
                let count = diags.iter().filter(|x| x.code() == d.code() && x.message == d.message).count();
                out.push(Edit { what: "widen-fix".into(), lint: d.code().into(), text: t, fixes: Some((d.message.clone(), count)), mono: false, wide: None, together: together(d.code(), alt - 1) });
            }
        }
    }
    out
}

// ---- the check ----

struct Ctx<'a> {
    args: &'a Args,
    test: &'a Test,
    dir: &'a Path,
    flags: Vec<String>,
}

impl Ctx<'_> {
    fn diagnose(&self, name: &str, text: &str, extra: &[&str], emit: &str) -> (Status, Vec<Diagnostic>) {
        let d = self.dir.join(name);
        let _ = std::fs::create_dir_all(&d);
        let src = d.join(self.test.file_name());
        let _ = std::fs::write(&src, text);
        let c = Compile::new(&self.args.rustc, &src, &d, &self.flags, self.test.edition())
            .emit(emit)
            .extra(extra.iter().copied())
            .json()
            .timeout(120)
            .run();
        (c.status, rustc::diagnostics(&c.stderr))
    }

    fn run(&self, name: &str, text: &str) -> Option<Observed> {
        let d = self.dir.join(name);
        let _ = std::fs::create_dir_all(&d);
        let src = d.join(self.test.file_name());
        let _ = std::fs::write(&src, text);
        let c = Compile::new(&self.args.rustc, &src, &d, &self.flags, self.test.edition()).extra(["-Awarnings"]).timeout(300).run();
        let mut o = observe(c.binary.as_ref()?, 30, &[]);
        o.stdout = normalize::stdout(&o.stdout);
        o.stderr = normalize::stderr(&o.stderr);
        Some(o)
    }
}

fn check(args: &Args, test: &Test) -> Rec {
    let mut rec = Rec { test: test.rel.clone(), skip: None, lints: Vec::new(), compiles: 0, found: Vec::new(), expected: Vec::new() };
    let skip = |rec: &mut Rec, why: &str| rec.skip = Some(why.into());
    if BY_PATH.is_match(&test.text) {
        skip(&mut rec, "uses files by path");
        return rec;
    }
    if test.text.contains('\r') || test.text.starts_with('\u{feff}') || (test.text.starts_with("#!") && !test.text.starts_with("#![")) {
        skip(&mut rec, "CR, BOM or shebang");
        return rec;
    }
    let dir = driver::scratch_dir(&args.sweep);
    let widen: Vec<String> = if args.no_widen { Vec::new() } else { WIDEN.iter().map(|l| format!("-W{l}")).collect() };
    let mut ctx = Ctx { args, test, dir: dir.path(), flags: test.flags.iter().cloned().chain(widen.iter().cloned()).collect() };
    let (mut status, mut diags) = ctx.diagnose("base", &test.text, &[], "metadata");
    rec.compiles += 1;
    if status == Status::Error && diags.iter().filter(|d| d.level == "error" && !SUMMARY.is_match(&d.message)).all(|d| is_lint(d.code())) {
        // The test denies its lints: back to warnings.
        ctx.flags.push("--cap-lints=warn".into());
        (status, diags) = ctx.diagnose("base", &test.text, &[], "metadata");
        rec.compiles += 1;
    }
    if status != Status::Ok {
        skip(&mut rec, &format!("original {status:?}").to_lowercase());
        return rec;
    }
    let lints: BTreeSet<String> = diags.iter().filter(|d| d.level == "warning" && is_lint(d.code())).map(|d| d.code().to_owned()).collect();
    rec.lints = lints.iter().cloned().collect();
    if lints.is_empty() {
        return rec;
    }
    let mut found: Vec<serde_json::Value> = Vec::new();
    let mut kept: Vec<(String, Vec<u8>)> = Vec::new();
    let base = keys(&diags, 0, test.file_name());

    // allow: removes that lint's warnings and nothing else.
    for lint in lints.iter().take(args.max_allow) {
        let text = format!("#![allow({lint})]\n{}", test.text);
        let (st, ad) = ctx.diagnose(&format!("allow-{lint}"), &text, &[], "metadata");
        rec.compiles += 1;
        let after = keys(&ad, 1, test.file_name());
        let other = |m: &BTreeMap<Key, usize>| -> BTreeMap<Key, usize> { m.iter().filter(|(k, _)| k.1 != *lint).map(|(k, v)| (k.clone(), *v)).collect() };
        let (b, a) = (other(&base), other(&after));
        let added: Vec<&Key> = a.iter().filter(|(k, v)| b.get(*k).is_none_or(|x| x < *v)).map(|(k, _)| k).collect();
        let removed: Vec<&Key> = b.iter().filter(|(k, v)| a.get(*k).is_none_or(|x| x < *v)).map(|(k, _)| k).collect();
        let what = if st == Status::Ice {
            Some("allow-ice")
        } else if st != Status::Ok {
            Some("allow-error")
        } else if !added.is_empty() || !removed.is_empty() {
            Some("allow-changes")
        } else {
            None
        };
        if let Some(what) = what {
            let name = format!("allow-{lint}.rs");
            found.push(serde_json::json!({
                "what": what, "lint": lint, "file": name,
                "added": added.iter().take(5).map(|k| format!("{} {} {}: {}", k.0, k.1, k.3, k.2)).collect::<Vec<_>>(),
                "removed": removed.iter().take(5).map(|k| format!("{} {} {}: {}", k.0, k.1, k.3, k.2)).collect::<Vec<_>>(),
            }));
            kept.push((name, text.into_bytes()));
        }
    }

    // delete, premise and widen-fix edits.
    let ix = index(&test.text);
    proc_macro2::extra::invalidate_current_thread_spans();
    let Some(ix) = ix else {
        rec.found = summarize(&found);
        if !found.is_empty() {
            driver::write_finding(&args.sweep.work, test, &kept, &serde_json::json!({ "found": found }));
        }
        return rec;
    };
    let runnable = test.kind == Some(Kind::RunPass);
    let mut base_run: Option<Option<Observed>> = None;
    let base_errors = errors(&diags);
    for (i, e) in edits(args, &test.text, test.file_name(), &ix, &diags, NO_CORE.is_match(&test.text)).into_iter().enumerate() {
        let name = format!("{}-{}-{i}", e.what, e.lint);
        let mut e = e;
        // Lints the edit trips (the test's own `deny`s) are not what is checked.
        let mut extra: Vec<&str> = if ctx.flags.iter().any(|f| f.starts_with("--cap-lints")) { vec![] } else { vec!["--cap-lints=warn"] };
        if e.mono {
            extra.push("-Clink-dead-code=yes");
        }
        let emit = if e.mono { "obj" } else { "metadata" };
        if let Some((l, operand)) = e.wide.clone() {
            // Peel references until the assertion holds; a level that cannot be dereferenced
            // while the assertion still fails is a thin pointer.
            let mut verdict = None;
            for k in 0..4 {
                let t = apply(&test.text, vec![(l.clone(), format!("(*__mirth_wide(&({}({operand})))))", "*".repeat(k)))]).unwrap_or_default() + WIDE_HELPER;
                let (st, ed) = ctx.diagnose(&format!("{name}-{k}"), &t, &extra, emit);
                rec.compiles += 1;
                let failed = ed.iter().any(|d| d.level == "error" && d.message.contains("mirth: not a wide pointer"));
                if st == Status::Ok {
                    verdict = None;
                    break;
                }
                if failed {
                    e.text = t;
                    verdict = Some(k);
                    continue;
                }
                break; // cannot dereference further: the last failure stands
            }
            if let Some(k) = verdict {
                let file = format!("{name}.rs");
                found.push(serde_json::json!({ "what": "premise-breaks", "lint": e.lint, "file": file, "derefs": k, "first_error": "mirth: not a wide pointer" }));
                kept.push((file, e.text.into_bytes()));
            }
            continue;
        }
        let (st, ed) = ctx.diagnose(&name, &e.text, &extra, emit);
        rec.compiles += 1;
        let new: Vec<String> = errors(&ed).difference(&base_errors).cloned().collect();
        let mut what: Option<String> = None;
        let explained = (st == Status::Error)
            .then(|| {
                EXPLAINED.iter().find(|(lint, needle, codes, _)| {
                    *lint == e.lint && test.text.contains(needle) && (codes.is_empty() || (!new.is_empty() && new.iter().all(|c| codes.contains(&c.as_str()))))
                })
            })
            .flatten();
        let together_ok = || {
            e.together.as_ref().is_some_and(|t| {
                let (st2, ed2) = ctx.diagnose(&format!("{name}-together"), t, &extra, emit);
                st2 == Status::Ok && errors(&ed2).difference(&base_errors).next().is_none()
            })
        };
        if let Some((_, _, _, why)) = explained {
            rec.expected.push(format!("{}: {why}", e.lint));
        } else if e.lint == "dead_code" && st == Status::Error && unresolved_within(&e.text, &ed, test.file_name(), |ix| ix.exempt.clone()) {
            // What dead code may use without being reported uses the deleted items.
            rec.expected.push("dead_code: used by exempt dead code".into());
        } else if e.lint == "dead_code" && st == Status::Error && unresolved_within(&e.text, &ed, test.file_name(), |ix| ix.uses.iter().map(|(r, _)| r.clone()).collect()) {
            // A `use` that is not itself a use (a re-export nothing reaches) names them.
            rec.expected.push("dead_code: re-exported by an unreachable import".into());
        } else if e.lint == "dead_code" && st == Status::Error && unresolved_within(&e.text, &ed, test.file_name(), |ix| ix.macros.clone()) {
            // Macro-generated code names the deleted items; the edit cannot see it.
            rec.expected.push("dead_code: used by macro-generated code".into());
        } else if st == Status::Ice {
            what = Some("ice".into());
        } else if e.what == "widen-fix" && st != Status::Ok && together_ok() {
            // This fix needs another fix of the same lint; together they compile.
            rec.expected.push(format!("{}: fixes depend on each other", e.lint));
        } else if st != Status::Ok || !new.is_empty() {
            what = Some(format!("{}-breaks", e.what));
        } else if let Some((msg, count)) = &e.fixes
            && ed.iter().filter(|x| x.code() == e.lint && x.message == *msg).count() >= *count
        {
            what = Some("widen-not-fixed".into());
        } else if runnable {
            let b = base_run.get_or_insert_with(|| {
                let a = ctx.run("run-base", &test.text);
                let b2 = ctx.run("run-base2", &test.text);
                if a == b2 { a } else { None }
            });
            if let Some(b) = b
                && let Some(o) = ctx.run(&format!("run-{name}"), &e.text)
                && o != *b
            {
                what = Some(format!("{}-changes-output", e.what));
            }
        }
        if let Some(what) = what {
            let file = format!("{name}.rs");
            found.push(serde_json::json!({
                "what": what, "lint": e.lint, "file": file,
                "new_errors": new.iter().take(5).collect::<Vec<_>>(),
                "first_error": ed.iter().find(|d| d.level == "error").map(|d| d.message.clone()),
            }));
            kept.push((file, e.text.into_bytes()));
        }
    }
    rec.found = summarize(&found);
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &kept, &serde_json::json!({ "found": found }));
    }
    rec
}

/// Every error of the edited file is a name that no longer resolves, inside the ranges `within`
/// picks: items dead code may use without being reported, or macros.
fn unresolved_within(text: &str, diags: &[Diagnostic], file: &str, within: impl Fn(&Index) -> Vec<Range>) -> bool {
    const UNRESOLVED: &[&str] = &["E0405", "E0412", "E0422", "E0423", "E0425", "E0432", "E0433", "E0531"];
    let Some(mut ix) = index(text) else { return false };
    proc_macro2::extra::invalidate_current_thread_spans();
    let impls: Vec<Range> = ix.impls.iter().filter(|(_, t, _)| ix.exempt_types.contains(t)).map(|(r, _, _)| r.clone()).collect();
    ix.exempt.extend(impls);
    let ranges = within(&ix);
    let errs: Vec<&Diagnostic> = diags.iter().filter(|d| d.level == "error" && !SUMMARY.is_match(&d.message)).collect();
    !errs.is_empty()
        && errs.iter().all(|d| {
            UNRESOLVED.contains(&d.code()) && spans(d, file).iter().all(|sp| ranges.iter().any(|r| r.start <= sp.start && sp.end <= r.end))
        })
}

fn summarize(found: &[serde_json::Value]) -> Vec<String> {
    found.iter().map(|f| format!("{}: {}", f["what"].as_str().unwrap_or(""), f["lint"].as_str().unwrap_or(""))).collect()
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let tests = uitest::tests(&args.sweep.tests, uitest::ALL, |_| false);
    let tests = args.sweep.select(tests);
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, t)))
}

//! Feature-gate mutation: inputs that reach the ICEs unstable features cause (checks.md Part 1:
//! 144 of the 334 crash bugs needed a nightly feature, 62 of them generic_const_args).
//!
//! The standalone UI tests are indexed by the features their `#![feature(…)]` enables;
//! incomplete features (from the checkout's rustc_feature) are picked three times as often.
//! Each mutant comes from one strategy:
//!
//! - `splice`: the items of a test using another feature, added to a test (flat, dropping names
//!   that collide, or inside a module), with both crates' gates;
//! - `move`: an item copied (impls moved) into a generic fn, an async fn, a closure, an anonymous
//!   const or a module; a free fn also into a trait (as a default method) or an inherent impl;
//!   or the whole file through mirth-rewrite's generic-wrap;
//! - `gate`: an extra incomplete feature enabled on a test using any feature;
//! - `edit`: one to three of the fuzzers' mechanical edits.
//!
//! Each mutant is compiled (`--emit=link`) under one configuration column: the test's own
//! flags, plus `-Znext-solver=globally` or `-Zassumptions-on-binders` for some. A finding is an
//! ICE or a hang (a timeout confirmed with three times the time) that the unmutated test does
//! not give. Findings are grouped by signature (the panic's source location and the first
//! query on the stack); each keeps its smallest mutant and a greedy reduction of it (top-level
//! items, then lines, while the signature stays). `--triage` searches rust-lang/rust's issues
//! for each signature's message.
//!
//! Mutant i is a function of `--seed` and i, so a run resumes where it stopped.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use mirth_lab::compiler_checks::{self, Known};
use mirth_lab::mutations;
use mirth_lab::rustc::{Compile, Status};
use mirth_lab::uitest::{self, Test};
use quote::{ToTokens, quote};
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use syn::Item;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// The rust checkout (tests/ui, and rustc_feature for the incomplete features).
    #[arg(long)]
    rust: PathBuf,
    #[arg(long)]
    work: PathBuf,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Mutants to make (resuming skips those already done).
    #[arg(long, default_value_t = 2000)]
    count: usize,
    #[arg(long, default_value_t = 4)]
    jobs: usize,
    /// Seconds per compilation.
    #[arg(long, default_value_t = 30)]
    timeout: u64,
    /// Compilations per reduction.
    #[arg(long, default_value_t = 300)]
    reduce_budget: usize,
    /// Signatures already known, one per line (a tab and a label may follow).
    #[arg(long)]
    known: Option<PathBuf>,
    /// Stop at the first new signature (exit 3).
    #[arg(long)]
    pause_on_finding: bool,
    /// Compile each mutant in an incremental session with the patched compiler's own checks on
    /// (RUSTC_VERIFY_REUSE=all, RUSTC_REPORT_UNTRACKED); what they report goes to
    /// <work>/compiler-checks.jsonl, with the mutant under <work>/compiler-checks/.
    #[arg(long)]
    compiler_checks: bool,
    /// Only features whose name contains this.
    #[arg(long)]
    only: Option<String>,
    /// Search rust-lang/rust's issues for each signature found so far, and stop.
    #[arg(long)]
    triage: bool,
    /// Reduce each signature's smallest mutant again (after the reducer changed), and stop.
    #[arg(long)]
    rereduce: bool,
}

static KNOWN: LazyLock<Known> = LazyLock::new(Known::load);
static FEATURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#!\[feature\(([^)\]]*)\)\]").unwrap());
static INCOMPLETE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*\(incomplete, (\w+),").unwrap());
/// Tests that are meant to crash, or whose output is already an ICE.
static EXPECTS_ICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"failure-status:\s*101|internal compiler error|rustc-ice|//@\s*should-ice").unwrap());

fn features(text: &str) -> BTreeSet<String> {
    FEATURE
        .captures_iter(text)
        .flat_map(|c| c[1].split(',').map(|f| f.trim().to_owned()).collect::<Vec<_>>())
        .filter(|f| !f.is_empty() && f.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect()
}

/// The pool to draw from: tests by feature, each feature weighted.
struct Corpus {
    tests: Vec<Test>,
    feature_names: Vec<String>,
    by_feature: Vec<Vec<usize>>,
    weights: rand::distr::weighted::WeightedIndex<u32>,
    incomplete: Vec<String>,
}

impl Corpus {
    fn load(args: &Args) -> anyhow::Result<Corpus> {
        let unstable = std::fs::read_to_string(args.rust.join("compiler/rustc_feature/src/unstable.rs")).unwrap_or_default();
        let incomplete: Vec<String> = INCOMPLETE.captures_iter(&unstable).map(|c| c[1].to_owned()).filter(|f| f != "test_incomplete_feature").collect();
        let tests = uitest::tests(&args.rust.join("tests/ui"), uitest::ALL, |t| features(&t.text).is_empty() || EXPECTS_ICE.is_match(&t.text));
        let mut index: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, t) in tests.iter().enumerate() {
            for f in features(&t.text) {
                if args.only.as_deref().is_none_or(|o| f.contains(o)) {
                    index.entry(f).or_default().push(i);
                }
            }
        }
        anyhow::ensure!(!index.is_empty(), "no tests enable a matching feature");
        let feature_names: Vec<String> = index.keys().cloned().collect();
        let weights = feature_names.iter().map(|f| if incomplete.contains(f) { 3 } else { 1 });
        let weights = rand::distr::weighted::WeightedIndex::new(weights)?;
        let by_feature = index.into_values().collect();
        Ok(Corpus { tests, feature_names, by_feature, weights, incomplete })
    }

    fn pick(&self, rng: &mut StdRng) -> (usize, &Test) {
        use rand::distr::Distribution;
        let f = self.weights.sample(rng);
        let i = *self.by_feature[f].choose(rng).unwrap();
        (f, &self.tests[i])
    }
}

// ---- strategies ----

/// The file pretty-printed, or as tokens where prettyplease has no printer for the syntax
/// (syn keeps newer syntax as verbatim tokens).
fn print(file: &syn::File) -> String {
    std::panic::catch_unwind(|| prettyplease::unparse(file)).unwrap_or_else(|_| file.to_token_stream().to_string())
}

fn item_ident(item: &Item) -> Option<String> {
    Some(
        match item {
            Item::Const(i) => &i.ident,
            Item::Enum(i) => &i.ident,
            Item::Fn(i) => &i.sig.ident,
            Item::Mod(i) => &i.ident,
            Item::Static(i) => &i.ident,
            Item::Struct(i) => &i.ident,
            Item::Trait(i) => &i.ident,
            Item::TraitAlias(i) => &i.ident,
            Item::Type(i) => &i.ident,
            Item::Union(i) => &i.ident,
            Item::Macro(i) => i.ident.as_ref()?,
            _ => return None,
        }
        .to_string(),
    )
}

fn is_main(item: &Item) -> bool {
    matches!(item, Item::Fn(f) if f.sig.ident == "main")
}

/// Crate-level attributes that may only appear once or only at the crate root.
fn crate_only(attr: &syn::Attribute) -> bool {
    let p = attr.path();
    ["feature", "crate_type", "crate_name", "no_std", "no_core", "no_main", "recursion_limit", "type_length_limit", "rustc_coherence_is_core"]
        .iter()
        .any(|n| p.is_ident(n))
}

fn splice(rng: &mut StdRng, a: &str, b: &str) -> Option<(String, &'static str)> {
    let mut fa = syn::parse_file(a).ok()?;
    let fb = syn::parse_file(b).ok()?;
    let mut have: BTreeSet<String> = fa.items.iter().filter_map(item_ident).collect();
    for attr in fb.attrs.iter().filter(|a| a.path().is_ident("feature")) {
        if !fa.attrs.iter().any(|x| x.to_token_stream().to_string() == attr.to_token_stream().to_string()) {
            fa.attrs.push(attr.clone());
        }
    }
    let items: Vec<Item> = fb.items.into_iter().filter(|i| !is_main(i)).collect();
    if items.is_empty() {
        return None;
    }
    if rng.random_bool(0.5) {
        for item in items {
            if let Some(n) = item_ident(&item)
                && !have.insert(n)
            {
                continue;
            }
            fa.items.push(item);
        }
        Some((print(&fa), "splice-flat"))
    } else {
        let lints: Vec<&syn::Attribute> = fb.attrs.iter().filter(|a| !crate_only(a)).collect();
        let lints = lints.iter().map(|a| {
            let meta = &a.meta;
            quote!(#![#meta])
        });
        fa.items.push(syn::parse_quote! { mod __mirth_spliced { #(#lints)* #(#items)* } });
        Some((print(&fa), "splice-mod"))
    }
}

const HOSTS: &[&str] = &["generic-fn", "async-fn", "closure", "anon-const", "module"];

fn host(kind: &str, item: &Item) -> Item {
    match kind {
        "generic-fn" => syn::parse_quote! { #[allow(unused)] fn __mirth_host<__MirthT>() { #item } },
        "async-fn" => syn::parse_quote! { #[allow(unused)] async fn __mirth_host() { #item } },
        "closure" => syn::parse_quote! { #[allow(unused)] fn __mirth_host() { let _ = || { #item }; } },
        "anon-const" => syn::parse_quote! { const _: () = { #item }; },
        _ => syn::parse_quote! { #[allow(unused)] mod __mirth_host { use super::*; #item } },
    }
}

fn movable(item: &Item) -> bool {
    !is_main(item) && !matches!(item, Item::Use(_) | Item::ExternCrate(_) | Item::Macro(_) | Item::Verbatim(_) | Item::ForeignMod(_))
}

fn relocate(rng: &mut StdRng, text: &str) -> Option<(String, String)> {
    if rng.random_ratio(1, 8) {
        return match mirth_rewrite::rewrite("generic-wrap", text) {
            mirth_rewrite::Outcome::Rewritten(t) => Some((syn::parse_file(&t).map(|f| print(&f)).unwrap_or(t), "move-generic-wrap".into())),
            _ => None,
        };
    }
    let mut file = syn::parse_file(text).ok()?;
    let candidates: Vec<usize> = (0..file.items.len()).filter(|&i| movable(&file.items[i])).collect();
    let &i = candidates.choose(rng)?;
    let item = file.items[i].clone();
    let mut kinds: Vec<&str> = HOSTS.to_vec();
    if matches!(item, Item::Fn(ref f) if f.block.stmts.len() < 200) {
        kinds.extend(["trait-default", "inherent"]);
    }
    let kind = *kinds.choose(rng)?;
    let wrapped: Item = match (kind, &item) {
        ("trait-default", Item::Fn(f)) => {
            let mut f = f.clone();
            f.vis = syn::Visibility::Inherited;
            let (attrs, sig, block) = (&f.attrs, &f.sig, &f.block);
            syn::parse_quote! { #[allow(unused)] trait __MirthTr { #(#attrs)* #sig #block } }
        }
        ("inherent", Item::Fn(f)) => syn::parse_quote! { #[allow(unused)] const _: () = { struct __MirthS; impl __MirthS { #f } }; },
        _ => host(kind, &item),
    };
    // Impls are global: a copy would conflict, so they move. Other items are copied, so what
    // names them still resolves.
    if matches!(item, Item::Impl(_)) {
        file.items[i] = wrapped;
    } else {
        file.items.insert(i + 1, wrapped);
    }
    Some((print(&file), format!("move-{kind}")))
}

fn add_gate(rng: &mut StdRng, text: &str, incomplete: &[String]) -> Option<(String, String)> {
    let have = features(text);
    let options: Vec<&String> = incomplete.iter().filter(|f| !have.contains(*f)).collect();
    let f = options.choose(rng)?;
    let at = FEATURE.find(text)?.start();
    Some((format!("{}#![feature({f})]\n{}", &text[..at], &text[at..]), format!("gate-{f}")))
}

fn edit(rng: &mut StdRng, text: &str, n: usize) -> Option<(String, String)> {
    let mut t = text.to_owned();
    let mut names = Vec::new();
    for k in 0..rng.random_range(1..=3) {
        let (name, f) = mutations::pick(rng);
        if let Some(next) = f(&t, rng, n * 4 + k) {
            t = next;
            names.push(name);
        }
    }
    (!names.is_empty()).then(|| (t, format!("edit-{}", names.join("+"))))
}

/// Configuration columns: the test's own flags, and the experimental solver flags.
const CONFIGS: &[(&str, &[&str], u32)] =
    &[("base", &[], 5), ("next-solver", &["-Znext-solver=globally"], 3), ("assumptions", &["-Zassumptions-on-binders"], 2)];

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Mutant {
    i: usize,
    strategy: String,
    sources: Vec<String>,
    config: String,
}

/// Mutant i: up to eight tries, since a strategy may not apply to the test drawn.
fn make(corpus: &Corpus, seed: u64, i: usize) -> Option<(Mutant, String, Vec<String>, String)> {
    (0..8u64).find_map(|attempt| make_one(corpus, seed, i, attempt))
}

fn make_one(corpus: &Corpus, seed: u64, i: usize, attempt: u64) -> Option<(Mutant, String, Vec<String>, String)> {
    let mut rng = StdRng::seed_from_u64(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ ((i as u64) << 3 | attempt));
    let (fa, a) = corpus.pick(&mut rng);
    let mut sources = vec![a.rel.clone()];
    let r = rng.random_range(0..100);
    let (text, strategy) = if r < 35 {
        // A partner using a different feature.
        let mut partner = None;
        for _ in 0..8 {
            let (fb, b) = corpus.pick(&mut rng);
            if fb != fa && b.rel != a.rel {
                partner = Some(b);
                break;
            }
        }
        let b = partner?;
        sources.push(b.rel.clone());
        let (t, s) = splice(&mut rng, &a.text, &b.text)?;
        (t, s.to_owned())
    } else if r < 65 {
        relocate(&mut rng, &a.text)?
    } else if r < 80 {
        add_gate(&mut rng, &a.text, &corpus.incomplete)?
    } else {
        edit(&mut rng, &a.text, i)?
    };
    use rand::distr::Distribution;
    static CW: LazyLock<rand::distr::weighted::WeightedIndex<u32>> =
        LazyLock::new(|| rand::distr::weighted::WeightedIndex::new(CONFIGS.iter().map(|c| c.2)).unwrap());
    let (config, extra, _) = CONFIGS[CW.sample(&mut rng)];
    let mut flags = a.flags.clone();
    flags.extend(extra.iter().map(|s| s.to_string()));
    Some((Mutant { i, strategy, sources, config: config.into() }, text, flags, a.edition().to_owned()))
}

// ---- signatures ----

static PANIC_AT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"panicked at (\S+?\.rs):(\d+):\d+:\n(.*)").unwrap());
static ICE_AT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"error: internal compiler error: (?:(\S+?\.rs):(\d+):\d+: )?(.*)").unwrap());
static DELAYED_AT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"delayed at (\S+?\.rs):(\d+):\d+").unwrap());
static QUERY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#0 \[(\w+)\]").unwrap());
static SIGNAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"rustc interrupted by (SIG\w+)").unwrap());
static QUOTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`]*`").unwrap());
static NUMBERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());
static OVERFLOW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"has overflowed its stack").unwrap());

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Signature {
    key: String,
    message: String,
}

fn signature(stderr: &str) -> Signature {
    let query = QUERY.captures(stderr).map(|c| format!(" [{}]", &c[1])).unwrap_or_default();
    let short = |p: &str| p.split("compiler/").last().unwrap_or(p).to_owned();
    if OVERFLOW.is_match(stderr) {
        return Signature { key: format!("stack overflow{query}"), message: "rustc has overflowed its stack".into() };
    }
    if let Some(c) = PANIC_AT.captures(stderr) {
        // `bug!` panics with a non-string payload; its text is on the ICE line.
        let message = match ICE_AT.captures(stderr) {
            Some(i) if c[3].trim() == "Box<dyn Any>" => i[3].trim().to_owned(),
            _ => c[3].trim().to_owned(),
        };
        return Signature { key: format!("{}:{}{query}", short(&c[1]), &c[2]), message: message.chars().take(300).collect() };
    }
    if let Some(c) = ICE_AT.captures(stderr) {
        let loc = match (c.get(1), DELAYED_AT.captures(stderr)) {
            (Some(f), _) => format!("{}:{}", short(f.as_str()), &c[2]),
            (None, Some(d)) => format!("{}:{}", short(&d[1]), &d[2]),
            // A delayed bug: the location is only in the backtrace, so the message names it.
            (None, None) => format!("delayed: {}", NUMBERS.replace_all(&QUOTED.replace_all(c[3].trim(), "`_`"), "N")),
        };
        return Signature { key: format!("{loc}{query}"), message: c[3].trim().chars().take(300).collect() };
    }
    if let Some(c) = SIGNAL.captures(stderr) {
        return Signature { key: format!("{}{query}", &c[1]), message: c[0].to_owned() };
    }
    Signature { key: format!("ice{query}"), message: stderr.lines().find(|l| l.contains("error")).unwrap_or("").chars().take(300).collect() }
}

// ---- compiling and reducing ----

struct Env<'a> {
    args: &'a Args,
    scratch: PathBuf,
}

/// Status and signature (for an ICE, or "hang" for a confirmed timeout), and whether the
/// compiler complained of a missing feature gate.
fn compile(env: &Env, text: &str, flags: &[String], edition: &str) -> (Status, Option<Signature>, bool) {
    let (status, sig, ungated, _) = compile_checked(env, text, flags, edition, false);
    (status, sig, ungated)
}

/// `compile`, and with `checks` what the patched compiler's own checks reported.
fn compile_checked(env: &Env, text: &str, flags: &[String], edition: &str, checks: bool) -> (Status, Option<Signature>, bool, Vec<String>) {
    let d = tempfile::tempdir_in(&env.scratch).expect("scratch");
    let src = d.path().join("m.rs");
    let _ = std::fs::write(&src, text);
    let first = Compile::new(&env.args.rustc, &src, d.path(), flags, edition).emit("link").timeout(env.args.timeout);
    let first = if checks { first.compiler_checks(&d.path().join("incr"), true) } else { first };
    let c = first.run();
    let reported = if checks { compiler_checks::read(&c.stderr, &KNOWN).findings() } else { Vec::new() };
    let (status, sig, ungated) = classify(env, &src, d.path(), flags, edition, c);
    (status, sig, ungated, reported)
}

fn classify(env: &Env, src: &Path, dir: &Path, flags: &[String], edition: &str, c: mirth_lab::rustc::Compiled) -> (Status, Option<Signature>, bool) {
    let run = |secs| Compile::new(&env.args.rustc, src, dir, flags, edition).emit("link").timeout(secs).run();
    let ungated = c.stderr.contains("E0658");
    match c.status {
        Status::Ice => (Status::Ice, Some(signature(&c.stderr)), ungated),
        Status::Timeout => match run(env.args.timeout * 3) {
            t if t.status == Status::Timeout => {
                (Status::Timeout, Some(Signature { key: "hang".into(), message: format!("no result in {}s", env.args.timeout * 3) }), ungated)
            }
            t if t.status == Status::Ice => (Status::Ice, Some(signature(&t.stderr)), t.stderr.contains("E0658")),
            t => (t.status, None, t.stderr.contains("E0658")),
        },
        s => (s, None, ungated),
    }
}

/// A reduction step keeps the signature, and does not reach it through a missing feature
/// gate when the input did not (an ICE after "feature not enabled" is another bug).
struct Reducer<'a> {
    env: &'a Env<'a>,
    flags: &'a [String],
    edition: &'a str,
    key: &'a str,
    ungated_ok: bool,
    budget: usize,
}

impl Reducer<'_> {
    fn same(&mut self, text: &str) -> bool {
        if self.budget == 0 {
            return false;
        }
        self.budget -= 1;
        let (_, sig, ungated) = compile(self.env, text, self.flags, self.edition);
        sig.is_some_and(|s| s.key == self.key) && (self.ungated_ok || !ungated)
    }

    /// Top-level items, when syn parses the file.
    fn items(&mut self, best: &mut String) {
        let Ok(mut cur) = syn::parse_file(best) else { return };
        let mut k = cur.items.len();
        while k > 0 && self.budget > 0 {
            k -= 1;
            let mut trial = cur.clone();
            trial.items.remove(k);
            let t = print(&trial);
            if self.same(&t) {
                cur = trial;
                *best = t;
            }
        }
    }

    /// Runs of lines: halves, quarters, … down to single lines, and every brace-balanced block
    /// (a line that opens braces through the line that closes them).
    fn lines(&mut self, best: &mut String) {
        let mut lines: Vec<String> = best.lines().map(str::to_owned).collect();
        let try_cut = |this: &mut Self, lines: &mut Vec<String>, a: usize, b: usize| -> bool {
            if b > lines.len() || a >= b {
                return false;
            }
            let mut trial = lines.clone();
            trial.drain(a..b);
            if this.same(&(trial.join("\n") + "\n")) {
                *lines = trial;
                true
            } else {
                false
            }
        };
        // Brace-balanced blocks, last first, so indices stay valid.
        let mut i = lines.len();
        while i > 0 && self.budget > 0 {
            i -= 1;
            let opens = lines[i].matches('{').count() as i64 - lines[i].matches('}').count() as i64;
            if opens <= 0 {
                continue;
            }
            let mut depth = 0i64;
            let mut j = i;
            while j < lines.len() {
                depth += lines[j].matches('{').count() as i64 - lines[j].matches('}').count() as i64;
                if depth <= 0 {
                    break;
                }
                j += 1;
            }
            if j < lines.len() {
                try_cut(self, &mut lines, i, j + 1);
            }
        }
        let mut size = (lines.len() / 2).max(1);
        loop {
            let mut a = lines.len();
            while a > 0 && self.budget > 0 {
                let start = a.saturating_sub(size);
                if lines[start..a].iter().any(|l| !l.trim().is_empty()) {
                    try_cut(self, &mut lines, start, a);
                }
                a = start;
            }
            if size == 1 || self.budget == 0 {
                break;
            }
            size /= 2;
        }
        *best = lines.join("\n") + "\n";
    }
}

/// Greedy reduction: top-level items, then blocks and runs of lines, until nothing shrinks.
fn reduce(env: &Env, text: &str, flags: &[String], edition: &str, key: &str) -> String {
    let ungated_ok = compile(env, text, flags, edition).2;
    let mut r = Reducer { env, flags, edition, key, ungated_ok, budget: env.args.reduce_budget };
    let mut best = text.to_owned();
    loop {
        let before = best.len();
        r.items(&mut best);
        r.lines(&mut best);
        if best.len() >= before || r.budget == 0 {
            return best;
        }
    }
}

// ---- the run ----

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Group {
    key: String,
    message: String,
    count: usize,
    first: Mutant,
    smallest: usize,
    dir: String,
    #[serde(default)]
    label: String,
    /// The command that reproduces the reduced file alone (`--triage`), or why none does.
    #[serde(default)]
    repro: String,
}

#[derive(Serialize)]
struct Line {
    #[serde(flatten)]
    mutant: Mutant,
    status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
}

fn short_hash(s: &str) -> String {
    mirth_lab::artifacts::sha256(s.as_bytes())[..10].to_owned()
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    // prettyplease's panics on syntax it cannot print are caught (see `print`): keep them quiet.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !info.location().is_some_and(|l| l.file().contains("prettyplease")) {
            default_hook(info);
        }
    }));
    std::fs::create_dir_all(args.work.join("findings"))?;
    let groups_path = args.work.join("signatures.json");
    let groups: BTreeMap<String, Group> =
        std::fs::read_to_string(&groups_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if args.triage {
        return triage(&args, groups);
    }
    if args.rereduce {
        let scratch = args.work.join("scratch");
        std::fs::create_dir_all(&scratch)?;
        let env = Env { args: &args, scratch };
        let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).stack_size(256 << 20).build()?;
        pool.install(|| {
            groups.values().collect::<Vec<_>>().par_iter().for_each(|g| {
                let dir = args.work.join("findings").join(&g.dir);
                let (Ok(text), Ok(detail)) = (std::fs::read_to_string(dir.join("mutant.rs")), std::fs::read_to_string(dir.join("finding.json"))) else { return };
                let detail: serde_json::Value = serde_json::from_str(&detail).unwrap_or_default();
                let flags: Vec<String> = detail["flags"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_owned)).collect();
                let edition = detail["edition"].as_str().unwrap_or("2015");
                let reduced = reduce(&env, &text, &flags, edition, &g.key);
                println!("{:60} {} -> {} bytes", g.key, text.len(), reduced.len());
                let _ = std::fs::write(dir.join("reduced.rs"), reduced);
            })
        });
        return Ok(ExitCode::SUCCESS);
    }
    let known: HashMap<String, String> = args
        .known
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (k, label) = l.split_once('\t').unwrap_or((l, "known"));
            (k.trim().to_owned(), label.trim().to_owned())
        })
        .collect();
    let corpus = Corpus::load(&args)?;
    let log_path = args.work.join("results.jsonl");
    let done: BTreeSet<usize> = std::fs::read_to_string(&log_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()?["i"].as_u64().map(|i| i as usize))
        .collect();
    let pending: Vec<usize> = (0..args.count).filter(|i| !done.contains(i)).collect();
    println!(
        "{} tests over {} features ({} incomplete); {} mutants to make, {} done",
        corpus.tests.len(),
        corpus.feature_names.len(),
        corpus.incomplete.len(),
        pending.len(),
        done.len()
    );
    let scratch = args.work.join("scratch");
    std::fs::create_dir_all(&scratch)?;
    let env = Env { args: &args, scratch };
    let log = Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(&log_path)?);
    let groups = Mutex::new(groups);
    let baselines: Mutex<HashMap<(String, String), Option<String>>> = Mutex::new(HashMap::new());
    let stop = AtomicBool::new(false);
    let progress = AtomicUsize::new(0);
    let (ices, new_groups) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).stack_size(256 << 20).build()?;
    pool.install(|| {
        pending.par_iter().for_each(|&i| {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let Some((mutant, text, flags, edition)) = make(&corpus, args.seed, i) else {
                let mut l = log.lock().unwrap();
                let _ = writeln!(l, "{}", serde_json::json!({"i": i, "strategy": "none"}));
                return;
            };
            let (status, sig, _, reported) = compile_checked(&env, &text, &flags, &edition, args.compiler_checks);
            if !reported.is_empty() {
                let dir = args.work.join("compiler-checks");
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::fs::write(dir.join(format!("{i}.rs")), &text);
                let line = serde_json::json!({ "i": i, "strategy": mutant.strategy, "sources": mutant.sources, "config": mutant.config, "flags": flags, "edition": edition, "found": reported });
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(args.work.join("compiler-checks.jsonl")) {
                    let _ = writeln!(f, "{line}");
                }
            }
            let mut key = None;
            if let Some(sig) = sig {
                // Not a finding when the unmutated test already gives it.
                let base_key = (mutant.sources[0].clone(), mutant.config.clone());
                let cached = baselines.lock().unwrap().get(&base_key).cloned();
                let base = cached.unwrap_or_else(|| {
                    let t = corpus.tests.iter().find(|t| t.rel == mutant.sources[0]).unwrap();
                    let b = compile(&env, &t.text, &flags, &edition).1.map(|s| s.key);
                    baselines.lock().unwrap().insert(base_key, b.clone());
                    b
                });
                if base.as_deref() != Some(sig.key.as_str()) {
                    ices.fetch_add(1, Ordering::Relaxed);
                    key = Some(sig.key.clone());
                    record(&env, &groups, &groups_path, &known, &mutant, &text, &flags, &edition, &sig, &new_groups, &stop);
                }
            }
            let line = Line { mutant, status, key };
            let mut l = log.lock().unwrap();
            let _ = writeln!(l, "{}", serde_json::to_string(&line).unwrap());
            drop(l);
            let n = progress.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 100 == 0 {
                println!("{n}/{} made, {} ICEs or hangs, {} signatures new this run", pending.len(), ices.load(Ordering::Relaxed), new_groups.load(Ordering::Relaxed));
            }
        })
    });
    let groups = groups.into_inner().unwrap();
    println!("{} signatures:", groups.len());
    for g in groups.values() {
        println!("  {:5} {:60} {}", g.count, g.key, g.message.chars().take(90).collect::<String>());
    }
    if stop.load(Ordering::Relaxed) {
        return Ok(ExitCode::from(3));
    }
    Ok(ExitCode::SUCCESS)
}

#[allow(clippy::too_many_arguments)]
fn record(
    env: &Env,
    groups: &Mutex<BTreeMap<String, Group>>,
    path: &Path,
    known: &HashMap<String, String>,
    mutant: &Mutant,
    text: &str,
    flags: &[String],
    edition: &str,
    sig: &Signature,
    new_groups: &AtomicUsize,
    stop: &AtomicBool,
) {
    let dir = env.args.work.join("findings").join(short_hash(&sig.key));
    let (is_new, smaller) = {
        let mut g = groups.lock().unwrap();
        let is_new = !g.contains_key(&sig.key);
        let entry = g.entry(sig.key.clone()).or_insert_with(|| Group {
            key: sig.key.clone(),
            message: sig.message.clone(),
            count: 0,
            first: mutant.clone(),
            smallest: usize::MAX,
            dir: dir.file_name().unwrap().to_string_lossy().into_owned(),
            label: known.get(&sig.key).cloned().unwrap_or_default(),
            repro: String::new(),
        });
        entry.count += 1;
        let smaller = text.len() < entry.smallest;
        if smaller {
            entry.smallest = text.len();
        }
        let _ = std::fs::write(path, serde_json::to_string_pretty(&*g).unwrap());
        (is_new, smaller)
    };
    if !smaller {
        return;
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("mutant.rs"), text);
    let detail = serde_json::json!({"signature": sig, "mutant": mutant, "flags": flags, "edition": edition});
    let _ = std::fs::write(dir.join("finding.json"), serde_json::to_string_pretty(&detail).unwrap());
    if is_new {
        let reduced = reduce(env, text, flags, edition, &sig.key);
        let _ = std::fs::write(dir.join("reduced.rs"), &reduced);
        let unknown = !known.contains_key(&sig.key);
        println!(
            "FINDING {} {} ({}, {}, {} -> {} bytes){}",
            sig.key,
            sig.message.chars().take(100).collect::<String>(),
            mutant.strategy,
            mutant.config,
            text.len(),
            reduced.len(),
            if unknown { "" } else { " [known]" }
        );
        if unknown {
            new_groups.fetch_add(1, Ordering::Relaxed);
            if env.args.pause_on_finding {
                let _ = std::fs::write(env.args.work.join("PAUSED"), &sig.key);
                stop.store(true, Ordering::Relaxed);
            }
        }
    }
}

// ---- triage ----

#[derive(Deserialize)]
struct Hit {
    number: u64,
    title: String,
    state: String,
}

fn search_words(message: &str) -> String {
    // Words that survive GitHub's tokenizer: no punctuation, no paths, no numbers.
    let words: Vec<String> = message
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() > 2 && !w.chars().all(|c| c.is_ascii_digit()) && !w.starts_with("DefId"))
        .take(8)
        .map(str::to_owned)
        .collect();
    words.join(" ")
}

static TEST_ATTR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[(test|bench)\b").unwrap());

/// The flags with which the reduced file gives the signature on its own: the mutant's flags,
/// else with `--test` added when the file or its source test uses the test harness.
fn reproduce(args: &Args, g: &mut Group) -> String {
    let dir = args.work.join("findings").join(&g.dir);
    let (Ok(text), Ok(detail)) = (std::fs::read_to_string(dir.join("reduced.rs")), std::fs::read_to_string(dir.join("finding.json"))) else {
        return "no reduced file".into();
    };
    let detail: serde_json::Value = serde_json::from_str(&detail).unwrap_or_default();
    let flags: Vec<String> = detail["flags"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_owned)).collect();
    let edition = detail["edition"].as_str().unwrap_or("2015").to_owned();
    let source = g.first.sources.first().map(|s| std::fs::read_to_string(args.rust.join("tests/ui").join(s)).unwrap_or_default()).unwrap_or_default();
    let mut tries = vec![flags.clone()];
    if !flags.iter().any(|f| f == "--test") && (TEST_ATTR.is_match(&text) || TEST_ATTR.is_match(&source) || source.contains("--test")) {
        tries.push([flags.clone(), vec!["--test".to_owned()]].concat());
    }
    let scratch = args.work.join("scratch");
    let _ = std::fs::create_dir_all(&scratch);
    let env = Env { args, scratch };
    for f in tries {
        if let Some(sig) = compile(&env, &text, &f, &edition).1.filter(|s| s.key == g.key) {
            g.message = sig.message;
            return format!("RUSTC_BOOTSTRAP=1 rustc reduced.rs --edition {edition} {}", f.join(" ")).trim().to_owned();
        }
    }
    "does not reproduce alone (the mutant does)".into()
}

fn triage(args: &Args, mut groups: BTreeMap<String, Group>) -> anyhow::Result<ExitCode> {
    let mut out = String::new();
    for g in groups.values_mut() {
        g.repro = reproduce(args, g);
        let loc_file = g.key.split(':').next().unwrap_or("").rsplit('/').next().unwrap_or("").to_owned();
        let mut hits: Vec<Hit> = Vec::new();
        for q in [search_words(&g.message), format!("{} {}", loc_file.trim_end_matches(".rs"), search_words(&g.message).split(' ').take(3).collect::<Vec<_>>().join(" "))] {
            if q.trim().is_empty() {
                continue;
            }
            let o = Command::new("gh")
                .args(["search", "issues", "--repo", "rust-lang/rust", "--json", "number,title,state", "--limit", "5", "--", &q])
                .env("GH_HOST", "github.com")
                .output()?;
            hits = serde_json::from_slice(&o.stdout).unwrap_or_default();
            if !hits.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
        g.label = match hits.iter().find(|h| h.state == "open").or(hits.first()) {
            Some(h) => format!("candidates: {}", hits.iter().map(|h| format!("#{} ({}) {}", h.number, h.state, h.title.chars().take(70).collect::<String>())).collect::<Vec<_>>().join("; ")).chars().take(600).collect::<String>() + &format!(" [best #{}]", h.number),
            None => "looks-new (no issue matched the message)".into(),
        };
        let _ = writeln!(out, "{:60} {:5} {}\n    {}\n    repro: {}", g.key, g.count, g.message.chars().take(100).collect::<String>(), g.label, g.repro);
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    std::fs::write(args.work.join("signatures.json"), serde_json::to_string_pretty(&groups)?)?;
    std::fs::write(args.work.join("triage.txt"), &out)?;
    print!("{out}");
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures() {
        let p = "thread 'rustc' panicked at compiler/rustc_middle/src/ty/x.rs:12:5:\nbad thing 3\nquery stack during panic:\n#0 [typeck] type-checking `f`\n#1 [x] y";
        let s = signature(p);
        assert_eq!(s.key, "rustc_middle/src/ty/x.rs:12 [typeck]");
        assert_eq!(s.message, "bad thing 3");
        let d = "error: internal compiler error: compiler/rustc_hir_typeck/src/a.rs:3:9: no type\n";
        assert_eq!(signature(d).key, "rustc_hir_typeck/src/a.rs:3");
        let delayed = "m.rs:21:15: error: internal compiler error: explicit deref of `T` 3 times\n";
        assert_eq!(signature(delayed).key, "delayed: explicit deref of `_` N times");
        assert_eq!(features("#![feature(a, b_c)]\n#![feature(d)]").into_iter().collect::<Vec<_>>(), ["a", "b_c", "d"]);
    }

    #[test]
    fn strategies_produce_code() {
        let a = "#![feature(x)]\nstruct A;\nimpl A { fn f(&self) {} }\nfn g() -> u8 { 1 }\nfn main() {}\n";
        let b = "#![feature(y)]\n#![allow(dead_code)]\nstruct A;\nfn h() {}\nfn main() {}\n";
        let mut rng = StdRng::seed_from_u64(1);
        for _ in 0..20 {
            let (t, _) = splice(&mut rng, a, b).unwrap();
            assert!(t.contains("feature(y)") && syn::parse_file(&t).is_ok());
            if let Some((t, _)) = relocate(&mut rng, a) {
                assert!(syn::parse_file(&t).is_ok(), "{t}");
            }
        }
    }
}

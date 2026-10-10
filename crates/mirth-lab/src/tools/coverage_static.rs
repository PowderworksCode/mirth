//! The static denominators of docs/coverage-plan.md (milestone M1), counted from the compiler's
//! source with syn, and what today's block coverage already says about them (an estimate until
//! the arm-instrumented build of M2 measures arms exactly).
//!
//! - Branch arms: every arm of every decision in a function: `match` arms, `if` and its (possibly
//!   implicit) `else`, `while`/`for` body and exit, `let ... else`, `?` (continue, return) and
//!   `&&`/`||` (evaluate the right side, skip it). An arm with a body is measurable from block
//!   sites when a site starts inside the body; implicit arms (no body: the `else` an `if` lacks,
//!   `?`'s early return) are not, until M2.
//! - Configuration branches: decisions whose condition reads a feature gate, a `-Z`/`-C` option,
//!   the edition or a target property.
//! - Feature-gate check sites: each call of a `Features` accessor (or `enabled(sym::…)`), per
//!   unstable feature.
//! - Delayed-bug sites: each `span_delayed_bug`/`delayed_bug` call.
//! - Keyed engine sites: queries (and their flags) × engine blocks.
//! - Metadata tables: `define_tables!`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::sitemap::{self, Inputs, Pos, SiteMap};
use quote::ToTokens;
use regex::Regex;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[command(flatten)]
    inputs: Inputs,
    /// Write the gap lists (arms never taken, configuration decisions seen one way, delayed-bug
    /// sites never reached) to this directory.
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Kind {
    Match,
    If,
    Loop,
    LetElse,
    Try,
    ShortCircuit,
}

struct Arm {
    /// The arm's body; None for an implicit arm.
    span: Option<(Pos, Pos)>,
}

struct Decision {
    kind: Kind,
    file: String,
    cond: (Pos, Pos),
    floor: Pos,
    arms: Vec<Arm>,
    config: Vec<(&'static str, String)>,
}

struct Call {
    file: String,
    span: (Pos, Pos),
    floor: Pos,
    name: String,
}

fn pos(s: proc_macro2::LineColumn) -> Pos {
    (s.line as u32, s.column as u32 + 1)
}

fn range(span: proc_macro2::Span) -> (Pos, Pos) {
    (pos(span.start()), pos(span.end()))
}

static FEATURE_LIST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(\s*(unstable|incomplete|internal)\s*,\s*(\w+)\s*,").unwrap());
static OPTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:unstable_opts|cg)\s*\.\s*(?:read_)?(\w+)").unwrap());
static EDITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bedition\s*\(\s*\)|\b(?:at_least|is)_rust_20\d\d\b|Edition\s*::\s*Edition20\d\d").unwrap());
static TARGET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\btarget\s*\.\s*(?:options\s*\.\s*)?(\w+)").unwrap());
// `gate!(visitor, name, …)` (any first argument) and `gate_all!(name, …)`.
static GATE_MACRO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^,]*,\s*(\w+)\s*,").unwrap());
static GATE_ALL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\w+)\s*,").unwrap());
static FEATURE_CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"features\s*(?:\(\s*\))?\s*\.\s*(\w+)\s*\(\s*\)|\benabled\s*\(\s*sym\s*::\s*(\w+)\s*\)").unwrap());

struct Scan<'a> {
    file: String,
    floor: Pos,
    features: &'a HashSet<String>,
    decisions: Vec<Decision>,
    feature_calls: Vec<Call>,
    delayed: Vec<Call>,
}

impl Scan<'_> {
    fn config(&self, cond: &str) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        for c in FEATURE_CALL.captures_iter(cond) {
            let name = c.get(1).or(c.get(2)).unwrap().as_str();
            if self.features.contains(name) {
                out.push(("feature", name.to_owned()));
            }
        }
        out.extend(OPTION.captures_iter(cond).map(|c| ("option", c[1].to_owned())));
        if EDITION.is_match(cond) {
            out.push(("edition", "edition".into()));
        }
        if cond.contains("sess") || cond.contains("tcx") {
            out.extend(TARGET.captures_iter(cond).map(|c| ("target", c[1].to_owned())));
        }
        out.sort();
        out.dedup();
        out
    }

    fn decide(&mut self, kind: Kind, cond: proc_macro2::Span, cond_text: String, arms: Vec<Arm>) {
        let config = if matches!(kind, Kind::Try) { Vec::new() } else { self.config(&cond_text) };
        self.decisions.push(Decision { kind, file: self.file.clone(), cond: range(cond), floor: self.floor, arms, config });
    }

    fn with_floor(&mut self, at: proc_macro2::Span, f: impl FnOnce(&mut Self)) {
        let saved = self.floor;
        self.floor = pos(at.start());
        f(self);
        self.floor = saved;
    }
}

fn body(span: proc_macro2::Span) -> Arm {
    Arm { span: Some(range(span)) }
}

const IMPLICIT: Arm = Arm { span: None };

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.with_floor(i.span(), |s| visit::visit_item_fn(s, i));
    }
    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        self.with_floor(i.span(), |s| visit::visit_impl_item_fn(s, i));
    }
    fn visit_trait_item_fn(&mut self, i: &'ast syn::TraitItemFn) {
        self.with_floor(i.span(), |s| visit::visit_trait_item_fn(s, i));
    }

    fn visit_expr_match(&mut self, e: &'ast syn::ExprMatch) {
        let arms = e.arms.iter().map(|a| body(a.body.span())).collect();
        self.decide(Kind::Match, e.expr.span(), e.expr.to_token_stream().to_string(), arms);
        visit::visit_expr_match(self, e);
    }
    fn visit_expr_if(&mut self, e: &'ast syn::ExprIf) {
        let mut arms = vec![body(e.then_branch.span())];
        arms.push(match &e.else_branch {
            Some((_, els)) => body(els.span()),
            None => IMPLICIT,
        });
        self.decide(Kind::If, e.cond.span(), e.cond.to_token_stream().to_string(), arms);
        visit::visit_expr_if(self, e);
    }
    fn visit_expr_while(&mut self, e: &'ast syn::ExprWhile) {
        self.decide(Kind::Loop, e.cond.span(), e.cond.to_token_stream().to_string(), vec![body(e.body.span()), IMPLICIT]);
        visit::visit_expr_while(self, e);
    }
    fn visit_expr_for_loop(&mut self, e: &'ast syn::ExprForLoop) {
        self.decide(Kind::Loop, e.expr.span(), e.expr.to_token_stream().to_string(), vec![body(e.body.span()), IMPLICIT]);
        visit::visit_expr_for_loop(self, e);
    }
    fn visit_local(&mut self, l: &'ast syn::Local) {
        if let Some(init) = &l.init
            && let Some((_, els)) = &init.diverge
        {
            self.decide(Kind::LetElse, init.expr.span(), init.expr.to_token_stream().to_string(), vec![IMPLICIT, body(els.span())]);
        }
        visit::visit_local(self, l);
    }
    fn visit_expr_try(&mut self, e: &'ast syn::ExprTry) {
        self.decide(Kind::Try, e.span(), String::new(), vec![IMPLICIT, IMPLICIT]);
        visit::visit_expr_try(self, e);
    }
    fn visit_expr_binary(&mut self, e: &'ast syn::ExprBinary) {
        if matches!(e.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            self.decide(Kind::ShortCircuit, e.left.span(), e.left.to_token_stream().to_string(), vec![body(e.right.span()), IMPLICIT]);
        }
        visit::visit_expr_binary(self, e);
    }
    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        // Gates checked through macros: `gate!(self, name, …)`, `gate_all!(name, …)`, and accessor
        // calls written inside any macro's tokens.
        let name = m.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        let tokens = m.tokens.to_string();
        let mut found: Vec<String> = Vec::new();
        if name.starts_with("gate") {
            let re = if name.starts_with("gate_all") { &*GATE_ALL } else { &*GATE_MACRO };
            found.extend(re.captures(&tokens).map(|c| c[1].to_owned()));
        }
        for c in FEATURE_CALL.captures_iter(&tokens) {
            found.push(c.get(1).or(c.get(2)).unwrap().as_str().to_owned());
        }
        for f in found.into_iter().filter(|f| self.features.contains(f)) {
            self.feature_calls.push(Call { file: self.file.clone(), span: range(m.span()), floor: self.floor, name: f });
        }
        visit::visit_macro(self, m);
    }

    fn visit_expr_method_call(&mut self, e: &'ast syn::ExprMethodCall) {
        let name = e.method.to_string();
        if name == "span_delayed_bug" || name == "delayed_bug" {
            self.delayed.push(Call { file: self.file.clone(), span: range(e.span()), floor: self.floor, name: name.clone() });
        }
        let text = e.to_token_stream().to_string();
        // The accessor call itself: `<…features…>.name()` or `<…>.enabled(sym::name)`.
        let feature = if e.args.is_empty() && self.features.contains(&name) && e.receiver.to_token_stream().to_string().contains("features") {
            Some(name)
        } else if name == "enabled" {
            FEATURE_CALL.captures(&text).and_then(|c| c.get(2)).map(|m| m.as_str().to_owned()).filter(|f| self.features.contains(f))
        } else {
            None
        };
        if let Some(f) = feature {
            self.feature_calls.push(Call { file: self.file.clone(), span: range(e.span()), floor: self.floor, name: f });
        }
        visit::visit_expr_method_call(self, e);
    }
}

/// The queries of `rustc_middle/src/queries.rs` with their modifiers.
fn queries(source: &str) -> Vec<(String, Vec<String>)> {
    static QUERY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*query\s+(\w+)\s*\(").unwrap());
    static MODIFIER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(eval_always|cache_on_disk_if|cache_on_disk|feedable|anon|no_hash|separate_provide_extern|depth_limit|cycle_\w+|return_result_from_ensure_ok|arena_cache)\b").unwrap()
    });
    let starts: Vec<(usize, String)> = QUERY.captures_iter(source).map(|c| (c.get(0).unwrap().start(), c[1].to_owned())).collect();
    starts
        .iter()
        .enumerate()
        .map(|(k, (at, name))| {
            let end = starts.get(k + 1).map_or(source.len(), |n| n.0);
            let mods = MODIFIER.captures_iter(&source[*at..end]).map(|c| c[1].to_owned()).collect();
            (name.clone(), mods)
        })
        .collect()
}

/// The tables of `define_tables!` in `rustc_metadata/src/rmeta/mod.rs`: (name, defaulted).
pub fn rmeta_tables(source: &str) -> Vec<(String, bool)> {
    static TABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*(\w+)\s*:\s*Table<").unwrap());
    let Some(start) = source.find("define_tables!") else { return Vec::new() };
    let body = &source[start..];
    let end = body.find("\n}\n").unwrap_or(body.len());
    let body = &body[..end];
    let optional = body.find("- optional:").unwrap_or(body.len());
    TABLE.captures_iter(body).map(|c| (c[1].to_owned(), c.get(0).unwrap().start() < optional)).collect()
}

#[derive(Default, serde::Serialize)]
struct Totals {
    decisions: usize,
    arms: usize,
    explicit: usize,
    implicit: usize,
    reachable_arms: usize,
    measurable: usize,
    panic_only: usize,
    taken: usize,
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let inp = &args.inputs;
    let map = SiteMap::load(&inp.sites);
    let reached = sitemap::reached(inp);
    let unreachable = sitemap::unreachable(&inp.gaps);
    anyhow::ensure!(!map.sites.is_empty(), "no site tables in {}", inp.sites.display());
    anyhow::ensure!(!unreachable.is_empty(), "no unreachable list in {} (run mirth-lab callgraph --json)", inp.gaps.display());
    let features: HashSet<String> = std::fs::read_to_string(inp.rust.join("compiler/rustc_feature/src/unstable.rs"))
        .map(|s| FEATURE_LIST.captures_iter(&s).map(|c| c[2].to_owned()).collect())
        .unwrap_or_default();

    let mut scan = Scan { file: String::new(), floor: (0, 0), features: &features, decisions: vec![], feature_calls: vec![], delayed: vec![] };
    let (mut parsed, mut skipped_moved, mut skipped_parse) = (0, Vec::new(), 0);
    static SYM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:sym|Features)\s*::\s*(\w+)|\bunstable!\s*\(\s*(\w+)").unwrap());
    let mut referenced: HashSet<String> = HashSet::new();
    for (key, path) in sitemap::compiler_files(&inp.rust) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if !key.ends_with("rustc_feature/src/unstable.rs") && !key.contains("rustc_span/src/symbol") {
            referenced.extend(SYM.captures_iter(&text).filter_map(|c| c.get(1).or(c.get(2)).map(|m| m.as_str().to_owned())).filter(|n| features.contains(n)));
        }
        if map.alignment(&key, &text) < 0.9 {
            skipped_moved.push(key);
            continue;
        }
        let Ok(file) = syn::parse_file(&text) else {
            skipped_parse += 1;
            continue;
        };
        scan.file = key;
        scan.floor = (0, 0);
        scan.visit_file(&file);
        parsed += 1;
    }

    let live = |s: &sitemap::Site| !unreachable.contains(&s.path);
    let mut out = String::new();
    let _ = writeln!(out, "{parsed} source files scanned; {} skipped (moved since the coverage build: {:?}); {skipped_parse} not parsed", skipped_moved.len(), skipped_moved);
    let blocks_live = map.sites.iter().filter(|s| live(s) && !s.log).count();
    let _ = writeln!(out, "{} sites in the tables; {} in reachable functions (outside logging macros); {} reached by the suites", map.sites.len(), blocks_live, map.sites.iter().filter(|s| reached.contains(&s.id)).count());

    // Branch arms.
    let mut per_kind: BTreeMap<Kind, Totals> = BTreeMap::new();
    let mut arm_gaps: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut all = Totals::default();
    for d in &scan.decisions {
        let cond_sites = map.probe(&d.file, d.cond.0, d.cond.1, d.floor);
        let reachable = cond_sites.first().is_some_and(|s| live(s));
        for t in [per_kind.entry(d.kind).or_default(), &mut all] {
            t.decisions += 1;
        }
        for arm in &d.arms {
            let inside: Vec<&sitemap::Site> = match arm.span {
                Some((a, b)) => map.probe(&d.file, a, b, (u32::MAX, 0)),
                None => Vec::new(),
            };
            let arm_live = inside.first().map_or(reachable, |s| live(s));
            let measurable = !inside.is_empty();
            let panics = measurable && inside.iter().all(|s| s.panics || s.log);
            let taken = inside.iter().any(|s| reached.contains(&s.id));
            for t in [per_kind.entry(d.kind).or_default(), &mut all] {
                t.arms += 1;
                if arm.span.is_some() { t.explicit += 1 } else { t.implicit += 1 }
                if arm_live {
                    t.reachable_arms += 1;
                    if panics {
                        t.panic_only += 1;
                    } else if measurable {
                        t.measurable += 1;
                        t.taken += taken as usize;
                    }
                }
            }
            if arm_live && measurable && !panics && !taken {
                let krate = d.file.split('/').nth(1).unwrap_or("").to_owned();
                let (line, _) = arm.span.unwrap().0;
                arm_gaps.entry((krate, d.file.clone())).or_default().push(format!("- {line} {:?} arm: `{}`", d.kind, inside[0].snippet.chars().take(80).collect::<String>()));
            }
        }
    }
    let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
    let _ = writeln!(out, "\nbranch arms: {} decisions, {} arms ({} with a body, {} implicit)", all.decisions, all.arms, all.explicit, all.implicit);
    let _ = writeln!(
        out,
        "  in reachable functions: {} arms; {} measurable from block sites (a site starts in the body), {} panic-only; taken {} ({:.1}%); implicit arms wait for M2",
        all.reachable_arms, all.measurable, all.panic_only, all.taken, pct(all.taken, all.measurable)
    );
    let _ = writeln!(out, "  arms per reachable site: {:.2} (the plan estimated about 0.5 new arm sites per block)", all.reachable_arms as f64 / blocks_live.max(1) as f64);
    let _ = writeln!(out, "  {:14} {:>9} {:>9} {:>10} {:>9} {:>7}", "kind", "decisions", "arms", "measurable", "taken", "%");
    for (k, t) in &per_kind {
        let _ = writeln!(out, "  {:14} {:9} {:9} {:10} {:9} {:7.1}", format!("{k:?}"), t.decisions, t.arms, t.measurable, t.taken, pct(t.taken, t.measurable));
    }

    // Configuration branches.
    #[derive(Default)]
    struct Cfg {
        decisions: usize,
        consulted: usize,
        first: usize,
        other: usize,
        both: usize,
    }
    let mut cfg: BTreeMap<(&str, String), Cfg> = BTreeMap::new();
    let mut cfg_gaps: Vec<String> = Vec::new();
    for d in scan.decisions.iter().filter(|d| !d.config.is_empty()) {
        let cond_sites = map.probe(&d.file, d.cond.0, d.cond.1, d.floor);
        if !cond_sites.first().is_some_and(|s| live(s)) {
            continue;
        }
        let consulted = cond_sites.iter().any(|s| reached.contains(&s.id));
        let arm_taken = |i: usize| -> Option<bool> {
            let (a, b) = d.arms.get(i)?.span?;
            let inside = map.probe(&d.file, a, b, (u32::MAX, 0));
            (!inside.is_empty()).then(|| inside.iter().any(|s| reached.contains(&s.id)))
        };
        let (first, other) = (arm_taken(0).unwrap_or(false), (1..d.arms.len()).any(|i| arm_taken(i).unwrap_or(false)));
        for c in &d.config {
            let e = cfg.entry((c.0, c.1.clone())).or_default();
            e.decisions += 1;
            e.consulted += consulted as usize;
            e.first += first as usize;
            e.other += other as usize;
            e.both += (first && other) as usize;
        }
        if consulted && !(first && other) {
            let (line, _) = d.cond.0;
            let names: Vec<String> = d.config.iter().map(|c| format!("{} {}", c.0, c.1)).collect();
            cfg_gaps.push(format!("- {}:{line} {:?} on {}: {}", d.file, d.kind, names.join(", "), if first { "only the first arm seen" } else if other { "only the other arms seen" } else { "consulted, no arm body reached" }));
        }
    }
    let mut by_kind: BTreeMap<&str, (usize, usize, usize, usize)> = BTreeMap::new();
    for ((k, _), c) in &cfg {
        let e = by_kind.entry(k).or_default();
        e.0 += 1;
        e.1 += c.decisions;
        e.2 += c.consulted;
        e.3 += c.both;
    }
    let _ = writeln!(out, "\nconfiguration branches (decisions in reachable functions whose condition reads a gate, option, edition or target property):");
    let _ = writeln!(out, "  {:9} {:>6} {:>10} {:>10} {:>16}", "kind", "names", "decisions", "consulted", "both arms seen");
    for (k, (n, d, c, b)) in &by_kind {
        let _ = writeln!(out, "  {k:9} {n:6} {d:10} {c:10} {b:16}");
    }

    // Feature-gate check sites.
    let mut gate: BTreeMap<String, (usize, usize)> = features.iter().map(|f| (f.clone(), (0, 0))).collect();
    for c in &scan.feature_calls {
        let sites = map.probe(&c.file, c.span.0, c.span.1, c.floor);
        let e = gate.entry(c.name.clone()).or_default();
        e.0 += 1;
        e.1 += sites.iter().any(|s| reached.contains(&s.id)) as usize;
    }
    let generic: Vec<&String> = gate.iter().filter(|(f, v)| v.0 == 0 && referenced.contains(*f)).map(|(f, _)| f).collect();
    let unchecked: Vec<&String> = gate.iter().filter(|(f, v)| v.0 == 0 && !referenced.contains(*f)).map(|(f, _)| f).collect();
    let with_sites = gate.values().filter(|v| v.0 > 0).count();
    let consulted = gate.values().filter(|v| v.1 > 0).count();
    let _ = writeln!(
        out,
        "\nfeature gates: {} unstable features; {} have accessor call sites in the compiler ({} call sites); {} were consulted by a measured run (on or off: M4 records which); {} with call sites never consulted",
        features.len(),
        with_sites,
        scan.feature_calls.len(),
        consulted,
        with_sites - consulted
    );
    let _ = writeln!(
        out,
        "  {} more are checked only through generic code that names them (`sym::name` in a gate table or the parser's gated spans, `Features::name` pointers, an attribute parser's `unstable!(name)`): block coverage cannot say whether those were consulted; {} are not named in the compiler that way (a lint declaration's `@feature_gate` identifier, a library const-stability attribute, rustdoc, or nothing)",
        generic.len(),
        unchecked.len()
    );

    // Delayed bugs.
    let mut delayed_gaps: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let (mut d_live, mut d_reached) = (0, 0);
    for c in &scan.delayed {
        let sites = map.probe(&c.file, c.span.0, c.span.1, c.floor);
        if !sites.first().is_some_and(|s| live(s)) {
            continue;
        }
        d_live += 1;
        if sites.iter().any(|s| reached.contains(&s.id)) {
            d_reached += 1;
        } else {
            delayed_gaps.entry(c.file.split('/').nth(1).unwrap_or("").to_owned()).or_default().push(format!("- {}:{} `{}`", c.file, c.span.0 .0, c.name));
        }
    }
    let _ = writeln!(out, "\ndelayed-bug sites: {} calls; {d_live} in reachable functions; {d_reached} reached ({:.1}%)", scan.delayed.len(), pct(d_reached, d_live));

    // Keyed engine sites.
    let qsrc = std::fs::read_to_string(inp.rust.join("compiler/rustc_middle/src/queries.rs")).unwrap_or_default();
    let qs = queries(&qsrc);
    let count = |m: &str| qs.iter().filter(|(_, v)| v.iter().any(|x| x.starts_with(m))).count();
    let engine: Vec<&sitemap::Site> = map
        .sites
        .iter()
        .filter(|s| live(s) && (s.path.starts_with("rustc_query_impl::execution::") || s.path.starts_with("rustc_middle::dep_graph::graph::")))
        .collect();
    let disk = engine.iter().filter(|s| s.path.contains("load_from_disk") || s.path.contains("try_load")).count();
    let feed = engine.iter().filter(|s| s.path.contains("feed")).count();
    let cached = count("cache_on_disk");
    let feedable = count("feedable");
    let possible: usize = qs
        .iter()
        .map(|(_, m)| {
            engine.len() - if m.iter().any(|x| x.starts_with("cache_on_disk")) { 0 } else { disk } - if m.iter().any(|x| x == "feedable") { 0 } else { feed }
        })
        .sum();
    let _ = writeln!(
        out,
        "\nkeyed engine sites: {} queries ({} eval_always, {cached} cached on disk, {feedable} feedable, {} anon); {} engine sites in reachable functions ({disk} on the load-from-disk path, {feed} on the feeding path); query × allowed engine site: {possible} possible keyed sites",
        qs.len(),
        count("eval_always"),
        count("anon"),
        engine.len()
    );

    // Metadata tables.
    let tables = rmeta_tables(&std::fs::read_to_string(inp.rust.join("compiler/rustc_metadata/src/rmeta/mod.rs")).unwrap_or_default());
    let _ = writeln!(out, "\nmetadata tables: {} in define_tables! ({} defaulted, {} optional)", tables.len(), tables.iter().filter(|t| t.1).count(), tables.iter().filter(|t| !t.1).count());
    print!("{out}");

    if let Some(dir) = &args.out {
        std::fs::create_dir_all(dir)?;
        let total: usize = arm_gaps.values().map(Vec::len).sum();
        let mut text = format!("# Arms never taken (estimate from block sites, M1): {total}\n\nArms with a body in reachable functions, not panic-only, where no site inside the body ran.\n\n");
        let mut by_crate: HashMap<&str, usize> = HashMap::new();
        for ((k, _), v) in &arm_gaps {
            *by_crate.entry(k).or_default() += v.len();
        }
        let mut crates: Vec<(&&str, &usize)> = by_crate.iter().collect();
        crates.sort_by(|a, b| b.1.cmp(a.1));
        for (krate, n) in crates {
            let _ = writeln!(text, "## {krate} ({n})\n");
            let mut files: Vec<(&(String, String), &Vec<String>)> = arm_gaps.iter().filter(|((k, _), _)| k == krate).collect();
            files.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
            for ((_, file), v) in files {
                let _ = writeln!(text, "### {file} ({})\n\n{}\n", v.len(), v.join("\n"));
            }
        }
        std::fs::write(dir.join("gaps-arms.md"), text)?;
        let mut text = format!("# Configuration decisions not seen both ways: {}\n\n", cfg_gaps.len());
        let _ = writeln!(text, "| kind | name | decisions | consulted | first arm | other arms | both |\n|---|---|---:|---:|---:|---:|---:|");
        for ((k, n), c) in &cfg {
            let _ = writeln!(text, "| {k} | {n} | {} | {} | {} | {} | {} |", c.decisions, c.consulted, c.first, c.other, c.both);
        }
        let _ = writeln!(text, "\n## Decisions\n\n{}", cfg_gaps.join("\n"));
        std::fs::write(dir.join("gaps-config.md"), text)?;
        let mut text = String::from("# Feature gates\n\n| feature | accessor call sites | consulted sites | checked |\n|---|---:|---:|---|\n");
        for (f, (n, r)) in &gate {
            let how = if *n > 0 { "directly" } else if referenced.contains(f) { "generically" } else { "not named in the compiler" };
            let _ = writeln!(text, "| {f} | {n} | {r} | {how} |");
        }
        std::fs::write(dir.join("gaps-features.md"), text)?;
        let mut text = format!("# Delayed-bug sites never reached: {}\n\n", d_live - d_reached);
        for (k, v) in &delayed_gaps {
            let _ = writeln!(text, "## {k} ({})\n\n{}\n", v.len(), v.join("\n"));
        }
        std::fs::write(dir.join("gaps-delayed.md"), text)?;
    }
    if let Some(j) = &args.json {
        let kinds: BTreeMap<String, &Totals> = per_kind.iter().map(|(k, v)| (format!("{k:?}"), v)).collect();
        std::fs::write(
            j,
            serde_json::to_string_pretty(&serde_json::json!({
                "arms": all, "arms_by_kind": kinds,
                "config_names": cfg.len(), "features": features.len(), "features_with_sites": with_sites, "features_consulted": consulted,
                "delayed_sites": scan.delayed.len(), "delayed_live": d_live, "delayed_reached": d_reached,
                "queries": qs.len(), "engine_sites": engine.len(), "keyed_possible": possible, "rmeta_tables": tables.len(),
                "live_sites": blocks_live,
            }))?,
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

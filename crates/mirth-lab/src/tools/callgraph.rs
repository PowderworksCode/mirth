//! Which of the compiler's functions can run at all: reachability over the call graph a compiler
//! built with rustc/callgraph.toml writes (`<crate>.graph`), against the functions (and blocks)
//! a compiler built with rustc/coverage.toml instruments. The functions that cannot be reached
//! leave coverage's denominator.
//!
//! The graph over-approximates what can run, so what it leaves out cannot run (as far as the
//! edges it knows go): direct calls, functions and closures used as values, callees MIR inlining
//! merged in, trait calls resolved in the caller; a call to a trait item reaches every body
//! implementing it, gated by rapid type analysis (a method of an impl for one of the compiler's
//! types only once reachable code builds the type, a trait impl's function only once reachable
//! code demands the trait for the impl's type). Roots: the compiler's and rustdoc's `main`s,
//! foreign-ABI functions, constant and static initializers, impls of traits from outside the
//! compiler, and what programs linking the compiler ran (`--external`).
//!
//! With hits, reports coverage of the reachable functions (and blocks), and checks the analysis:
//! a function that ran must be reachable.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::LazyLock;

use regex::Regex;
use walkdir::WalkDir;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The call-graph build's site directory (`<crate>.graph` files).
    #[arg(long)]
    graph: PathBuf,
    /// The coverage build's site directory (`<crate>.sites` files).
    #[arg(long)]
    sites: PathBuf,
    /// union.txt files of runs of the compiler.
    #[arg(long)]
    hit: Vec<PathBuf>,
    /// Directories of raw MIRTH_OUT logs.
    #[arg(long)]
    logs: Vec<PathBuf>,
    /// union.txt files of programs outside the compiler that link it: what they ran is a root.
    #[arg(long)]
    external: Vec<PathBuf>,
    #[arg(long)]
    json: Option<PathBuf>,
    /// List the unreachable functions of these crates.
    #[arg(long)]
    unreachable: Vec<String>,
    /// Print how the graph reaches these functions.
    #[arg(long)]
    why: Vec<String>,
    /// Write the reachable functions that never ran, by crate and file.
    #[arg(long)]
    gaps: Option<PathBuf>,
    /// Write the blocks that never ran in functions that did.
    #[arg(long)]
    block_gaps: Option<PathBuf>,
}

const ROOTS: &[&str] = &["rustc_main::main", "rustc_driver_impl::main", "rustdoc::main"];
/// Crates that do not run when the compiler does: proc macros, and a build-script helper.
const NOT_AT_RUN_TIME: &[&str] =
    &["rustc_macros", "rustc_type_ir_macros", "rustc_index_macros", "rustc_windows_rc", "rustc_hir_macros", "rustc_fluent_macro"];
const SCOPE: &[&str] = &["rustc_", "rustdoc"];
/// Called by the language on any value, not through a bound: drop glue.
const UNGATED_TRAITS: &[&str] = &["core::ops::drop::Drop"];

static PARENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.*)::\{[^}]*\}$").unwrap());
static CRATE_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-[0-9a-f]{16}$").unwrap());

type Node = String;
type Cond = (u8, String, String); // (0: live type, 1: demand (trait, type)), as strings

#[derive(Default)]
struct Graph {
    path_of: HashMap<Node, String>,
    hash_of: HashMap<String, Node>,
    implements: HashMap<Node, Node>,
    external_impl: HashSet<Node>,
    const_bodies: HashSet<Node>,
    self_type: HashMap<Node, Node>,
    constructs: HashMap<Node, HashSet<Node>>,
    spec_bounds: HashSet<Node>,
    ending: HashMap<Node, String>,
    diverges_into: HashMap<Node, HashSet<Node>>,
    impl_key: HashMap<Node, (Node, Node)>,
    demands: HashMap<Node, HashSet<(Node, Node)>>,
    edges: HashMap<Node, HashSet<Node>>,
}

fn load_graph(dir: &PathBuf) -> Graph {
    let mut g = Graph::default();
    let Ok(entries) = std::fs::read_dir(dir) else { return g };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "graph")).collect();
    files.sort();
    for f in files {
        let text = String::from_utf8_lossy(&std::fs::read(&f).unwrap_or_default()).into_owned();
        for line in text.lines() {
            let p: Vec<&str> = line.split('\t').collect();
            match p[0] {
                "body" if p.len() > 5 => {
                    let node = p[1].to_owned();
                    if p.len() > 7 && SCOPE.iter().any(|s| p[7].starts_with(s)) {
                        g.self_type.insert(node.clone(), p[6].to_owned());
                    }
                    // A trait with specializing impls: which impl a call reaches is decided by
                    // more than the bounds say, so such impls are not gated on them.
                    let specialized = p.len() > 11 && p[11] == "specialized";
                    if p.len() > 10 && p[8] != "-" && p[10] != "-" && !UNGATED_TRAITS.contains(&p[9]) && !specialized {
                        g.impl_key.insert(node.clone(), (p[8].to_owned(), p[10].to_owned()));
                    }
                    g.path_of.insert(node.clone(), p[2].to_owned());
                    g.hash_of.insert(p[2].to_owned(), node.clone());
                    if p.len() > 12 && (p[12] == "ice-only" || p[12] == "diverges") {
                        g.ending.insert(node.clone(), p[12].to_owned());
                    }
                    if p[5] == "const" || p[5] == "extern" {
                        g.const_bodies.insert(node.clone());
                    }
                    if p[3] != "-" {
                        g.implements.insert(node.clone(), p[3].to_owned());
                        if !p[4].starts_with("rustc_") {
                            g.external_impl.insert(node);
                        }
                    }
                }
                "diverges" if p.len() > 2 => {
                    g.diverges_into.entry(p[1].to_owned()).or_default().insert(p[2].to_owned());
                }
                "specbound" if p.len() > 1 => {
                    g.spec_bounds.insert(p[1].to_owned());
                }
                "demand" if p.len() > 3 => {
                    g.demands.entry(p[1].to_owned()).or_default().insert((p[2].to_owned(), p[3].to_owned()));
                }
                "edge" if p.len() > 3 => {
                    let map = if p[3] == "construct" { &mut g.constructs } else { &mut g.edges };
                    map.entry(p[1].to_owned()).or_default().insert(p[2].to_owned());
                }
                _ => {}
            }
        }
    }
    g
}

fn parent(path: &str) -> Option<String> {
    PARENT.captures(path).map(|c| c[1].to_owned())
}

struct Site {
    krate: String,
    path: String,
}

struct Block {
    krate: String,
    path: String,
    name: String,
    span: String,
    snippet: String,
    panics: bool,
    logging: bool,
}

fn load_sites(dir: &PathBuf) -> (BTreeMap<String, Site>, HashMap<String, String>, BTreeMap<String, Block>) {
    let (mut functions, mut span_of, mut blocks) = (BTreeMap::new(), HashMap::new(), BTreeMap::new());
    let Ok(entries) = std::fs::read_dir(dir) else { return (functions, span_of, blocks) };
    for e in entries.flatten() {
        if e.path().extension().is_none_or(|x| x != "sites") {
            continue;
        }
        let text = String::from_utf8_lossy(&std::fs::read(e.path()).unwrap_or_default()).into_owned();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 7 {
                continue;
            }
            // Newer tables name the crate with its stable id (`rustc_hash-<16 hex digits>`).
            let krate = CRATE_ID.replace(f[3], "").into_owned();
            match f[1] {
                "cover" => {
                    functions.insert(f[0].to_owned(), Site { krate, path: f[4].to_owned() });
                    span_of.insert(f[4].to_owned(), f[6].to_owned());
                }
                "block" => {
                    let mut parts = f[5].split(' ');
                    let name = parts.next().unwrap_or("").to_owned();
                    let tags: Vec<&str> = parts.collect();
                    blocks.insert(
                        f[0].to_owned(),
                        Block {
                            krate,
                            path: f[4].to_owned(),
                            name,
                            span: f[6].to_owned(),
                            snippet: f.get(7).unwrap_or(&"").to_string(),
                            panics: tags.contains(&"panics"),
                            logging: tags.contains(&"log"),
                        },
                    );
                }
                _ => {}
            }
        }
    }
    (functions, span_of, blocks)
}

fn words(path: &PathBuf) -> HashSet<String> {
    std::fs::read_to_string(path).unwrap_or_default().split_whitespace().map(str::to_owned).collect()
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let g = load_graph(&args.graph);
    let bodies: HashSet<&Node> = g.path_of.keys().collect();
    // A body runs only on a compiler bug when every path ends in a panic, directly or through
    // bodies that do.
    let mut ice_nodes: HashSet<Node> = g.ending.iter().filter(|(_, e)| *e == "ice-only").map(|(n, _)| n.clone()).collect();
    loop {
        let mut changed = false;
        for n in g.ending.keys() {
            if !ice_nodes.contains(n)
                && let Some(into) = g.diverges_into.get(n)
                && !into.is_empty()
                && into.iter().all(|x| ice_nodes.contains(x))
            {
                ice_nodes.insert(n.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let ice_only: HashSet<String> = ice_nodes.iter().filter_map(|n| g.path_of.get(n).cloned()).collect();
    let mut impl_key: HashMap<Node, (Node, Node)> = g.impl_key.iter().filter(|(_, k)| !g.spec_bounds.contains(&k.0)).map(|(n, k)| (n.clone(), k.clone())).collect();
    let mut implementors: HashMap<&Node, Vec<&Node>> = HashMap::new();
    for (body, item) in &g.implements {
        implementors.entry(item).or_default().push(body);
    }
    let mut roots: HashSet<Node> = ROOTS.iter().filter_map(|r| g.hash_of.get(*r).cloned()).collect();
    roots.extend(g.external_impl.iter().cloned());
    roots.extend(g.const_bodies.iter().cloned());
    let mut children: HashMap<Node, HashSet<Node>> = HashMap::new();
    for (node, path) in &g.path_of {
        let Some(mut up) = parent(path) else { continue };
        // Nested in a body, or in a static's or constant's initializer, which is not one here.
        while !g.hash_of.contains_key(&up) {
            match parent(&up) {
                Some(p) => up = p,
                None => break,
            }
        }
        match g.hash_of.get(&up) {
            Some(h) => {
                children.entry(h.clone()).or_default().insert(node.clone());
            }
            None => {
                roots.insert(node.clone());
            }
        }
    }
    let (functions, span_of, blocks) = load_sites(&args.sites);
    let mut external: HashSet<String> = HashSet::new();
    for u in &args.external {
        external.extend(words(u));
    }
    // Crates from crates.io that the scope takes in by name: other dependencies can name their
    // types and need their impls without any bound in the compiler saying so.
    let from_registry: HashSet<&str> = functions
        .values()
        .filter(|s| !span_of.get(&s.path).is_none_or(|sp| sp.starts_with("compiler/") || sp.starts_with("src/") || sp.starts_with("library/")))
        .map(|s| s.krate.as_str())
        .collect();
    impl_key.retain(|n, _| !g.path_of.get(n).is_some_and(|p| from_registry.contains(p.split("::").next().unwrap_or(""))));
    let external_roots: HashSet<Node> = external.iter().filter_map(|x| functions.get(x)).filter_map(|s| g.hash_of.get(&s.path).cloned()).collect();
    roots.extend(external_roots.iter().cloned());

    // Rapid type analysis.
    let mut reachable: HashSet<Node> = HashSet::new();
    let mut live: HashSet<Node> = HashSet::new();
    let mut demanded: HashSet<(Node, Node)> = HashSet::new();
    let mut came_from: HashMap<Node, (Option<Node>, &'static str)> = HashMap::new();
    let mut condition_from: HashMap<Cond, Node> = HashMap::new();
    let mut waiting: HashMap<Cond, HashSet<Node>> = HashMap::new();
    let mut queue: VecDeque<Node> = VecDeque::new();
    let missing = |node: &Node, live: &HashSet<Node>, demanded: &HashSet<(Node, Node)>| -> Option<Cond> {
        if let Some(t) = g.self_type.get(node)
            && !live.contains(t)
        {
            return Some((0, t.clone(), String::new()));
        }
        if let Some(k) = impl_key.get(node)
            && !demanded.contains(k)
        {
            return Some((1, k.0.clone(), k.1.clone()));
        }
        None
    };
    macro_rules! offer {
        ($node:expr, $gated:expr, $source:expr, $how:expr) => {{
            let node: Node = $node;
            if !reachable.contains(&node) {
                came_from.entry(node.clone()).or_insert(($source, $how));
                match if $gated { missing(&node, &live, &demanded) } else { None } {
                    Some(c) => {
                        waiting.entry(c).or_default().insert(node);
                    }
                    None => queue.push_back(node),
                }
            }
        }};
    }
    let mut sorted_roots: Vec<&Node> = roots.iter().collect();
    sorted_roots.sort();
    for r in sorted_roots {
        let gated = g.external_impl.contains(r) && !g.const_bodies.contains(r) && !external_roots.contains(r);
        offer!(r.clone(), gated, None, "root");
    }
    while let Some(node) = queue.pop_front() {
        if !reachable.insert(node.clone()) {
            continue;
        }
        let mut satisfied: Vec<Cond> = Vec::new();
        if let Some(ts) = g.constructs.get(&node) {
            for t in ts {
                if live.insert(t.clone()) {
                    let c = (0, t.clone(), String::new());
                    condition_from.insert(c.clone(), node.clone());
                    satisfied.push(c);
                }
            }
        }
        if let Some(ks) = g.demands.get(&node) {
            for k in ks {
                if demanded.insert(k.clone()) {
                    let c = (1, k.0.clone(), k.1.clone());
                    condition_from.insert(c.clone(), node.clone());
                    satisfied.push(c);
                }
            }
        }
        for c in satisfied {
            if let Some(nodes) = waiting.remove(&c) {
                for n in nodes {
                    offer!(n, true, None, "root");
                }
            }
        }
        let nexts: HashSet<&Node> = g.edges.get(&node).into_iter().flatten().chain(children.get(&node).into_iter().flatten()).collect();
        for nxt in nexts {
            offer!(nxt.clone(), false, Some(node.clone()), "edge");
            // A call to a trait item reaches the bodies implementing it.
            for imp in implementors.get(nxt).into_iter().flatten() {
                offer!((*imp).clone(), true, Some(node.clone()), "dispatch");
            }
        }
    }
    let reachable_paths: HashSet<&String> = reachable.iter().filter_map(|n| g.path_of.get(n)).collect();
    let mut out = String::new();
    for target in &args.why {
        let mut node = g.hash_of.get(target).cloned();
        let ok = node.as_ref().is_some_and(|n| reachable.contains(n));
        let _ = writeln!(out, "\nwhy {target}:{}", if ok { "" } else { " not reachable" });
        let mut seen = HashSet::new();
        while let Some(n) = node.clone() {
            if !reachable.contains(&n) || !seen.insert(n.clone()) {
                break;
            }
            let (source, how) = came_from.get(&n).cloned().unwrap_or((None, "?"));
            let mut extra = String::new();
            let conds = [g.self_type.get(&n).map(|t| (0u8, t.clone(), String::new())), impl_key.get(&n).map(|k| (1u8, k.0.clone(), k.1.clone()))];
            for c in conds.into_iter().flatten() {
                if let Some(from) = condition_from.get(&c) {
                    let _ = write!(extra, " [{} from {}]", if c.0 == 0 { "live" } else { "demand" }, g.path_of.get(from).unwrap_or(from));
                }
            }
            let _ = writeln!(out, "   {}  <- {how}{extra}", g.path_of.get(&n).unwrap_or(&n));
            node = source;
        }
    }
    let functions: BTreeMap<&String, &Site> = functions.iter().filter(|(_, s)| !NOT_AT_RUN_TIME.contains(&s.krate.as_str())).collect();
    let paths: HashSet<&String> = functions.values().map(|s| &s.path).collect();
    let known: HashSet<&String> = paths.iter().filter(|p| g.hash_of.contains_key(**p)).copied().collect();
    let unreach: HashSet<&String> = known.iter().filter(|p| !reachable_paths.contains(**p)).copied().collect();
    let _ = writeln!(out, "{} bodies in the graph, {} roots, {} reachable", bodies.len(), roots.len(), reachable.iter().filter(|n| bodies.contains(n)).count());
    let _ = writeln!(
        out,
        "{} instrumented functions; {} in the graph; {} unreachable ({:.1}%)",
        functions.len(),
        known.len(),
        unreach.len(),
        100.0 * unreach.len() as f64 / known.len().max(1) as f64
    );
    let mut hit: HashSet<String> = external.clone();
    for u in &args.hit {
        hit.extend(words(u));
    }
    for d in &args.logs {
        for e in WalkDir::new(d).into_iter().filter_map(Result::ok) {
            if e.path().extension().is_some_and(|x| x == "log") {
                for line in String::from_utf8_lossy(&std::fs::read(e.path()).unwrap_or_default()).lines() {
                    if let Some(s) = line.strip_prefix("V\t") {
                        hit.insert(s.to_owned());
                    }
                }
            }
        }
    }
    let hit_paths: HashSet<&String> = hit.iter().filter_map(|s| functions.get(s)).map(|s| &s.path).collect();
    let mut by_crate: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for s in functions.values() {
        let row = by_crate.entry(s.krate.as_str()).or_default();
        if unreach.contains(&s.path) {
            continue;
        }
        row.0 += 1;
        row.1 += hit_paths.contains(&s.path) as usize;
    }
    if !hit.is_empty() {
        let total: usize = by_crate.values().map(|r| r.0).sum();
        let ran: usize = by_crate.values().map(|r| r.1).sum();
        let _ = writeln!(out, "coverage: {} functions ran; of the {total} reachable ones, {ran} ({:.1}%)", hit_paths.len(), 100.0 * ran as f64 / total.max(1) as f64);
        let mut wrong: Vec<&&String> = hit_paths.iter().filter(|p| unreach.contains(**p)).collect();
        wrong.sort();
        let ice: HashSet<&String> = functions.values().map(|s| &s.path).filter(|p| ice_only.contains(*p) && !unreach.contains(p)).collect();
        let ice_ran = ice.iter().filter(|p| hit_paths.contains(**p)).count();
        let _ = writeln!(
            out,
            "of the reachable ones, {} only panic (they run on a compiler bug; {ice_ran} ran): without them, {} of {} ({:.1}%)",
            ice.len(),
            ran - ice_ran,
            total - ice.len(),
            100.0 * (ran - ice_ran) as f64 / (total - ice.len()).max(1) as f64
        );
        let _ = writeln!(out, "ran although unreachable (edges the analysis misses): {}", wrong.len());
        for w in wrong.iter().take(30) {
            let _ = writeln!(out, "    {w}");
        }
        if !blocks.is_empty() {
            // Blocks: each function's entry (its own site) and its other blocks, in reachable
            // functions; a block only panics when every path from it does, or its function does.
            let mut rows: BTreeMap<&str, [usize; 4]> = BTreeMap::new();
            for s in functions.values() {
                if !unreach.contains(&s.path) {
                    let r = rows.entry(s.krate.as_str()).or_default();
                    let (h, i) = (hit_paths.contains(&s.path), ice_only.contains(&s.path));
                    r[0] += 1;
                    r[1] += h as usize;
                    r[2] += i as usize;
                    r[3] += (i && h) as usize;
                }
            }
            let (mut logged, mut logged_ran) = (0, 0);
            for (site, b) in &blocks {
                if NOT_AT_RUN_TIME.contains(&b.krate.as_str()) || unreach.contains(&b.path) || !paths.contains(&b.path) {
                    continue;
                }
                if b.logging {
                    logged += 1;
                    logged_ran += hit.contains(site) as usize;
                    continue;
                }
                let r = rows.entry(b.krate.as_str()).or_default();
                let (h, p) = (hit.contains(site), b.panics || ice_only.contains(&b.path));
                r[0] += 1;
                r[1] += h as usize;
                r[2] += p as usize;
                r[3] += (p && h) as usize;
            }
            let s: [usize; 4] = rows.values().fold([0; 4], |a, r| [a[0] + r[0], a[1] + r[1], a[2] + r[2], a[3] + r[3]]);
            let _ = writeln!(
                out,
                "blocks: of the {} in reachable functions, {} ran ({:.1}%); {} only panic ({} ran): without them, {} of {} ({:.1}%); not counted: {logged} blocks of logging macros, which run only with RUSTC_LOG ({logged_ran} ran)",
                s[0], s[1], 100.0 * s[1] as f64 / s[0].max(1) as f64, s[2], s[3], s[1] - s[3], s[0] - s[2],
                100.0 * (s[1] - s[3]) as f64 / (s[0] - s[2]).max(1) as f64
            );
            let _ = writeln!(out, "{:40} {:>7} {:>9} {:>6}   (panic-only blocks aside)", "crate", "ran", "blocks", "%");
            let mut sorted: Vec<(&&str, &[usize; 4])> = rows.iter().collect();
            sorted.sort_by(|a, b| {
                let f = |r: &[usize; 4]| (r[1] - r[3]) as f64 / (r[0] - r[2]).max(1) as f64;
                f(a.1).partial_cmp(&f(b.1)).unwrap()
            });
            for (k, r) in sorted {
                if r[0] > r[2] {
                    let _ = writeln!(out, "{k:40} {:7} {:9} {:6.1}", r[1] - r[3], r[0] - r[2], 100.0 * (r[1] - r[3]) as f64 / (r[0] - r[2]) as f64);
                }
            }
        }
        let _ = writeln!(out, "{:40} {:>7} {:>9} {:>6}", "crate", "ran", "reachable", "%");
        let mut sorted: Vec<(&&str, &(usize, usize))> = by_crate.iter().collect();
        sorted.sort_by(|a, b| (a.1.1 as f64 / a.1.0.max(1) as f64).partial_cmp(&(b.1.1 as f64 / b.1.0.max(1) as f64)).unwrap());
        for (k, (n, r)) in sorted {
            if *n > 0 {
                let _ = writeln!(out, "{k:40} {r:7} {n:9} {:6.1}", 100.0 * *r as f64 / *n as f64);
            }
        }
    }
    for krate in &args.unreachable {
        let _ = writeln!(out, "\nunreachable in {krate}:");
        let mut list: Vec<&&String> = unreach.iter().filter(|p| p.starts_with(&format!("{krate}::"))).collect();
        list.sort();
        for p in list {
            let _ = writeln!(out, "    {p}");
        }
    }
    if let Some(j) = &args.json {
        let sorted = |set: Vec<&String>| -> Vec<String> {
            let mut v: Vec<String> = set.into_iter().cloned().collect();
            v.sort();
            v
        };
        let json = serde_json::json!({
            "unreachable": sorted(unreach.iter().copied().collect()),
            "ice_only": sorted(ice_only.iter().filter(|p| paths.contains(p)).collect()),
            "ran_unreachable": sorted(hit_paths.iter().filter(|p| unreach.contains(**p)).copied().collect()),
            "reachable_not_hit": sorted(paths.iter().filter(|p| !unreach.contains(**p) && !hit_paths.contains(**p)).copied().collect()),
        });
        std::fs::write(j, serde_json::to_string(&json)?)?;
    }
    if let Some(gp) = &args.gaps {
        let mut files: BTreeMap<(&str, String), Vec<(String, &String)>> = BTreeMap::new();
        for s in functions.values() {
            if !unreach.contains(&s.path) && !hit_paths.contains(&s.path) {
                let span = span_of.get(&s.path).cloned().unwrap_or_else(|| "?".into());
                let file = span.rsplitn(3, ':').last().unwrap_or("").to_owned();
                files.entry((s.krate.as_str(), file)).or_default().push((span, &s.path));
            }
        }
        std::fs::write(gp, gaps_text(&files, |span, path| {
            let tag = if ice_only.contains(*path) { " (only panics)" } else { "" };
            let line = span.rsplitn(3, ':').nth(1).unwrap_or("");
            format!("- `{path}` {line}{tag}")
        }, "Reachable functions that never ran"))?;
        let _ = writeln!(out, "gaps written to {}", gp.display());
    }
    if let Some(bp) = &args.block_gaps
        && !blocks.is_empty()
    {
        let mut files: BTreeMap<(&str, String), Vec<(String, String)>> = BTreeMap::new();
        for (site, b) in &blocks {
            if hit_paths.contains(&b.path) && !hit.contains(site) && !NOT_AT_RUN_TIME.contains(&b.krate.as_str()) && !b.logging {
                let file = b.span.rsplitn(3, ':').last().unwrap_or("").to_owned();
                let panics = b.panics || ice_only.contains(&b.path);
                let snippet: String = b.snippet.chars().take(100).collect();
                files.entry((b.krate.as_str(), file)).or_default().push((
                    b.span.clone(),
                    format!("`{}` {}{}: `{snippet}`", b.path, b.name, if panics { " (only panics)" } else { "" }),
                ));
            }
        }
        // The lines with the most first within a file.
        let line_no = |span: &str| -> (u64, u64) {
            let p: Vec<&str> = span.rsplitn(3, ':').collect();
            (p.get(1).and_then(|x| x.parse().ok()).unwrap_or(0), p.first().and_then(|x| x.parse().ok()).unwrap_or(0))
        };
        let mut text = format!("# Blocks that never ran in functions that did: {}\n\n", files.values().map(Vec::len).sum::<usize>());
        let mut by_crate: BTreeMap<&str, usize> = BTreeMap::new();
        for ((k, _), v) in &files {
            *by_crate.entry(k).or_default() += v.len();
        }
        let mut crates: Vec<(&&str, &usize)> = by_crate.iter().collect();
        crates.sort_by(|a, b| b.1.cmp(a.1));
        for (krate, n) in crates {
            let _ = writeln!(text, "## {krate} ({n})\n");
            let mut fs: Vec<(&(&str, String), &Vec<(String, String)>)> = files.iter().filter(|((k, _), _)| k == krate).collect();
            fs.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
            for ((_, file), entries) in fs {
                let _ = writeln!(text, "### {file} ({})\n", entries.len());
                let mut es = entries.clone();
                es.sort_by_key(|(span, _)| line_no(span));
                for (span, desc) in es {
                    let _ = writeln!(text, "- {} {desc}", line_no(&span).0);
                }
                text.push('\n');
            }
        }
        std::fs::write(bp, text)?;
        let _ = writeln!(out, "block gaps written to {}", bp.display());
    }
    print!("{out}");
    Ok(ExitCode::SUCCESS)
}

/// A gap list: by crate (largest first), then file (largest first), entries sorted by span.
fn gaps_text<P>(files: &BTreeMap<(&str, String), Vec<(String, P)>>, entry: impl Fn(&str, &P) -> String, title: &str) -> String {
    let total: usize = files.values().map(Vec::len).sum();
    let mut text = format!("# {title}: {total}\n\n");
    let mut by_crate: BTreeMap<&str, usize> = BTreeMap::new();
    for ((k, _), v) in files {
        *by_crate.entry(k).or_default() += v.len();
    }
    let mut crates: Vec<(&&str, &usize)> = by_crate.iter().collect();
    crates.sort_by(|a, b| b.1.cmp(a.1));
    for (krate, n) in crates {
        let _ = writeln!(text, "## {krate} ({n})\n");
        let mut fs: Vec<(&(&str, String), &Vec<(String, P)>)> = files.iter().filter(|((k, _), _)| k == krate).collect();
        fs.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        for ((_, file), entries) in fs {
            let _ = writeln!(text, "### {file} ({})\n", entries.len());
            let mut es: Vec<&(String, P)> = entries.iter().collect();
            es.sort_by(|a, b| a.0.cmp(&b.0));
            for (span, p) in es {
                let _ = writeln!(text, "{}", entry(span, p));
            }
            text.push('\n');
        }
    }
    text
}

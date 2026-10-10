//! Coverage beyond blocks (docs/coverage-plan.md): the dimensions a compiler built with
//! rustc/coverage-dims.toml and docs/hunt/coverage-dims.patch records, read from the suites'
//! keyed.txt, pairs.txt, passes.txt and locks.txt (coverage-compact folds them) against the site
//! tables. Arms and configuration arms are `mirth-lab callgraph`'s (`arms:`, `--arm-gaps`,
//! `--config`), since their denominator is the call graph's reachable functions.
//!
//! Writes one Markdown report:
//! - keyed engine coverage: per query (dep kind), the query engine's functions it entered and
//!   their blocks it took, against the blocks other queries took in the same functions;
//! - incremental transitions: per dep-node kind, which of the dependency graph's functions ran
//!   for it;
//! - feature gates consulted: each `Features::<feature>()` accessor, seen returning true, false,
//!   both, or never called;
//! - type kinds: per keyed function, the `TyKind`s that reached it;
//! - call pairs: (caller, callee) pairs of instrumented functions, against the call graph's edges
//!   between functions that ran;
//! - MIR pass effect: per pass and body kind, whether the pass ever changed a body;
//! - lock contention: the `Lock::lock` call sites found held under the parallel front end.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The site tables of the build (`<crate>.sites`).
    #[arg(long)]
    sites: PathBuf,
    /// Suite directories (each with union.txt and the files beside it); repeat.
    #[arg(long)]
    suite: Vec<PathBuf>,
    /// The call-graph build's `<crate>.graph` files, for the call-pair report.
    #[arg(long)]
    graph: Option<PathBuf>,
    #[arg(long)]
    out: PathBuf,
}

/// A keyed site: one block of a keyed function (or its return).
struct KeyedSite {
    function: String,
    block: String,
    label: String,
}

#[derive(Default)]
struct Tables {
    keyed: HashMap<u64, KeyedSite>,
    /// Enum path → discriminant → variant name.
    enums: HashMap<String, BTreeMap<u64, String>>,
    /// Function site → path.
    functions: HashMap<u64, String>,
    /// Paths of the instrumented functions.
    paths: BTreeSet<String>,
}

fn load(dir: &Path) -> Tables {
    let mut t = Tables::default();
    for table in mirth_lab::coverage::files_with(dir, "sites") {
        let text = String::from_utf8_lossy(&std::fs::read(&table).unwrap_or_default()).into_owned();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 7 {
                continue;
            }
            let Ok(id) = f[0].parse::<u64>() else { continue };
            match f[1] {
                "cover" => {
                    t.functions.insert(id, f[4].to_owned());
                    t.paths.insert(f[4].to_owned());
                }
                "keyed" => {
                    let (block, label) = f[5].split_once(' ').unwrap_or((f[5], ""));
                    t.keyed.insert(id, KeyedSite { function: f[4].to_owned(), block: block.to_owned(), label: label.to_owned() });
                }
                "keyenum" => {
                    let names = t.enums.entry(f[4].to_owned()).or_default();
                    for pair in f[5].split(',') {
                        if let Some((value, name)) = pair.split_once('=')
                            && let Ok(value) = value.parse::<u64>()
                        {
                            names.insert(value, name.to_owned());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    t
}

fn lines(dir: &Path, name: &str) -> Vec<String> {
    std::fs::read_to_string(dir.join(name)).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(str::to_owned).collect()
}

/// The last path segment, or the segment before a closure's.
fn short(path: &str) -> String {
    let parts: Vec<&str> = path.split("::").collect();
    match parts.as_slice() {
        [.., a, b] if b.starts_with('{') => format!("{a}::{b}"),
        [.., b] => b.to_string(),
        _ => path.to_owned(),
    }
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let t = load(&args.sites);
    // Gather the suites' records.
    let mut union: HashSet<u64> = HashSet::new();
    let mut keyed: BTreeSet<(u64, u64)> = BTreeSet::new();
    let mut pairs: HashSet<(u64, u64)> = HashSet::new();
    let mut passes: BTreeSet<(String, String, String, bool)> = BTreeSet::new();
    let mut locks: BTreeSet<String> = BTreeSet::new();
    for suite in &args.suite {
        union.extend(lines(suite, "union.txt").iter().filter_map(|l| l.parse::<u64>().ok()));
        for l in lines(suite, "keyed.txt") {
            if let Some((site, key)) = l.split_once('\t')
                && let (Ok(site), Ok(key)) = (site.parse(), key.parse())
            {
                keyed.insert((site, key));
            }
        }
        for l in lines(suite, "pairs.txt") {
            if let Some((a, b)) = l.split_once('\t')
                && let (Ok(a), Ok(b)) = (a.parse(), b.parse())
            {
                pairs.insert((a, b));
            }
        }
        for l in lines(suite, "passes.txt") {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() == 4 {
                passes.insert((f[0].to_owned(), f[1].to_owned(), f[2].to_owned(), f[3] == "changed"));
            }
        }
        locks.extend(lines(suite, "locks.txt"));
    }
    let ran: HashSet<&String> = t.functions.iter().filter(|(id, _)| union.contains(&(**id | 1)) || union.contains(id)).map(|(_, p)| p).collect();
    let mut out = String::from("# Coverage beyond blocks\n\n");
    let _ = writeln!(
        out,
        "Suites: {}. Read with `mirth-lab coverage-dims`; arms and configuration arms are in `mirth-lab callgraph`'s report.\n",
        args.suite.iter().map(|s| format!("`{}`", s.file_name().unwrap_or_default().to_string_lossy())).collect::<Vec<_>>().join(", ")
    );

    // Keyed sites by label: function → block → keys seen.
    let mut by_label: BTreeMap<&str, BTreeMap<&str, BTreeMap<&str, BTreeSet<u64>>>> = BTreeMap::new();
    let mut blocks_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for site in t.keyed.values() {
        blocks_of.entry(site.function.as_str()).or_default().insert(site.block.as_str());
    }
    for (site, key) in &keyed {
        let Some(k) = t.keyed.get(site) else { continue };
        by_label
            .entry(k.label.as_str())
            .or_default()
            .entry(k.function.as_str())
            .or_default()
            .entry(k.block.as_str())
            .or_default()
            .insert(*key);
    }
    let enum_for = |label: &str| -> Option<&BTreeMap<u64, String>> {
        match label {
            "query" | "dep-kind" => t.enums.iter().find(|(p, _)| p.ends_with("DepKind")).map(|(_, v)| v),
            "ty-kind" => t.enums.iter().find(|(p, _)| p.ends_with("TyKind")).map(|(_, v)| v),
            _ => None,
        }
    };
    let name = |label: &str, key: u64| -> String {
        enum_for(label).and_then(|names| names.get(&key).cloned()).unwrap_or_else(|| key.to_string())
    };

    // 1. Keyed engine coverage: per query.
    if let Some(functions) = by_label.get("query") {
        let mut per_key: BTreeMap<u64, (usize, usize, usize)> = BTreeMap::new(); // (functions entered, blocks taken, blocks others took there)
        for blocks in functions.values() {
            let all: BTreeSet<u64> = blocks.values().flatten().copied().collect();
            let any_block: BTreeSet<&str> = blocks.keys().copied().collect();
            for &key in &all {
                let taken: usize = blocks.values().filter(|keys| keys.contains(&key)).count();
                let row = per_key.entry(key).or_default();
                row.0 += 1;
                row.1 += taken;
                row.2 += any_block.len() - taken;
            }
        }
        let kinds = enum_for("query").map_or(0, |names| names.len());
        let functions_keyed = t.keyed.values().filter(|k| k.label == "query").map(|k| k.function.as_str()).collect::<BTreeSet<_>>().len();
        let _ = writeln!(out, "## Keyed engine coverage (per query)\n");
        let _ = writeln!(
            out,
            "{functions_keyed} functions of the query engine and its per-query plumbing are keyed by the query's dep kind. {} of {kinds} dep kinds were seen; {} engine blocks were taken by some query; summed over queries, {} (query, block) pairs were taken where block coverage counts {} blocks.\n",
            per_key.len(),
            functions.values().map(|b| b.len()).sum::<usize>(),
            per_key.values().map(|r| r.1).sum::<usize>(),
            functions.values().map(|b| b.len()).sum::<usize>(),
        );
        let mut rows: Vec<(&u64, &(usize, usize, usize))> = per_key.iter().collect();
        rows.sort_by(|a, b| b.1.2.cmp(&a.1.2).then(a.0.cmp(b.0)));
        let _ = writeln!(out, "The queries with the most engine paths other queries took and they never did (top 25):\n\n| query | engine functions entered | engine blocks taken | blocks other queries took there, this one never |\n|---|---:|---:|---:|");
        for (key, (f, b, gap)) in rows.iter().take(25) {
            let _ = writeln!(out, "| `{}` | {f} | {b} | {gap} |", name("query", **key));
        }
        if let Some(names) = enum_for("query") {
            let seen: BTreeSet<u64> = per_key.keys().copied().collect();
            let never: Vec<&String> = names.iter().filter(|(k, _)| !seen.contains(k)).map(|(_, v)| v).collect();
            let _ = writeln!(out, "\nDep kinds no keyed engine function ran for ({}): {}\n", never.len(), never.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", "));
        }
        // The engine's transitions per query: which of its paths each query took.
        let interesting = [
            "execute_job_incr",
            "execute_job_non_incr",
            "load_from_disk_or_invoke_provider_green",
            "ensure_can_skip_execution",
            "force_query_dep_node",
            "handle_cycle",
            "wait_for_query",
            "check_feedable_consistency",
        ];
        let columns: Vec<(&str, &BTreeMap<&str, BTreeSet<u64>>)> = functions
            .iter()
            .filter(|(f, _)| interesting.iter().any(|i| f.ends_with(&format!("::{i}"))))
            .map(|(f, b)| (*f, b))
            .collect();
        if !columns.is_empty() {
            let _ = writeln!(out, "Engine paths per query: for each query kind seen, whether it entered each path (✓).\n");
            let _ = write!(out, "| query |");
            for (f, _) in &columns {
                let _ = write!(out, " {} |", short(f));
            }
            let _ = write!(out, "\n|---|");
            for _ in &columns {
                let _ = write!(out, ":-:|");
            }
            out.push('\n');
            let mut counts = vec![0usize; columns.len()];
            for key in per_key.keys() {
                let mut row = format!("| `{}` |", name("query", *key));
                for (i, (_, blocks)) in columns.iter().enumerate() {
                    let entered = blocks.get("bb0").is_some_and(|keys| keys.contains(key));
                    counts[i] += entered as usize;
                    row.push_str(if entered { " ✓ |" } else { " |" });
                }
                let _ = writeln!(out, "{row}");
            }
            let _ = write!(out, "| **queries** |");
            for c in counts {
                let _ = write!(out, " {c} |");
            }
            out.push_str("\n\n");
        }
    }

    // 2. Incremental transitions: per dep-node kind, the dependency graph's functions.
    if let Some(functions) = by_label.get("dep-kind") {
        let _ = writeln!(out, "## Incremental transitions (per dep-node kind)\n");
        let mut kinds: BTreeMap<u64, BTreeMap<&str, usize>> = BTreeMap::new();
        for (f, blocks) in functions {
            for keys in blocks.values() {
                for key in keys {
                    *kinds.entry(*key).or_default().entry(*f).or_default() += 1;
                }
            }
        }
        let fn_names: BTreeSet<&str> = functions.keys().copied().collect();
        let _ = writeln!(
            out,
            "{} dependency-graph functions keyed by the node's kind; {} kinds seen. Cells: blocks taken for that kind / blocks of the function.\n",
            fn_names.len(),
            kinds.len()
        );
        let columns: Vec<&str> = fn_names.iter().copied().collect();
        let _ = write!(out, "| dep kind |");
        for f in &columns {
            let _ = write!(out, " {} |", short(f));
        }
        let _ = write!(out, "\n|---|");
        for _ in &columns {
            let _ = write!(out, "--:|");
        }
        out.push('\n');
        for (key, row) in &kinds {
            let _ = write!(out, "| `{}` |", name("dep-kind", *key));
            for f in &columns {
                let total = blocks_of.get(f).map_or(0, |b| b.len());
                match row.get(f) {
                    Some(n) => {
                        let _ = write!(out, " {n}/{total} |");
                    }
                    None => out.push_str(" |"),
                }
            }
            out.push('\n');
        }
        out.push('\n');
    }

    // 3. Feature gates consulted.
    {
        let accessors: BTreeSet<&String> = t
            .keyed
            .values()
            .filter(|k| k.label == "feature" && k.block == "ret")
            .map(|k| &k.function)
            .filter(|f| !["enabled", "incomplete", "internal"].iter().any(|n| f.ends_with(&format!("::{n}"))))
            .collect();
        if !accessors.is_empty() {
            let seen = by_label.get("feature");
            let (mut both, mut on, mut off, mut never) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for f in &accessors {
                let keys: BTreeSet<u64> = seen.and_then(|s| s.get(f.as_str())).map(|b| b.values().flatten().copied().collect()).unwrap_or_default();
                let feature = short(f);
                match (keys.contains(&1), keys.contains(&0)) {
                    (true, true) => both.push(feature),
                    (true, false) => on.push(feature),
                    (false, true) => off.push(feature),
                    (false, false) => never.push(feature),
                }
            }
            let _ = writeln!(out, "## Feature gates consulted\n");
            let _ = writeln!(
                out,
                "Each `Features::<feature>()` accessor, keyed by what it returned. {} accessors: consulted while on and off {}, only while on {}, only while off {}, never consulted {}.\n",
                accessors.len(),
                both.len(),
                on.len(),
                off.len(),
                never.len()
            );
            let list = |v: &[String]| v.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", ");
            let _ = writeln!(out, "**Consulted only while off** (code that checks the gate, reached, with the gate never on: what gate-mutate should turn on): {}\n", list(&off));
            let _ = writeln!(out, "**Consulted only while on**: {}\n", list(&on));
            let _ = writeln!(out, "**Never consulted**: {}\n", list(&never));
        }
    }

    // 4. Type kinds.
    if let Some(functions) = by_label.get("ty-kind") {
        let names = enum_for("ty-kind");
        let all_kinds: BTreeSet<u64> = functions.values().flat_map(|b| b.values().flatten().copied()).collect();
        let keyed_functions = t.keyed.values().filter(|k| k.label == "ty-kind").map(|k| k.function.as_str()).collect::<BTreeSet<_>>();
        let _ = writeln!(out, "## Type kinds\n");
        let _ = writeln!(
            out,
            "{} functions keyed by the `TyKind` of their first `Ty` argument; {} ran; {} of {} kinds reached at least one.\n",
            keyed_functions.len(),
            functions.len(),
            all_kinds.len(),
            names.map_or(0, |n| n.len())
        );
        if let Some(names) = names {
            let never: Vec<&String> = names.iter().filter(|(k, _)| !all_kinds.contains(k)).map(|(_, v)| v).collect();
            let _ = writeln!(out, "Kinds that reached none of them: {}\n", never.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", "));
        }
        let mut rows: Vec<(&str, BTreeSet<u64>)> = functions
            .iter()
            .map(|(f, b)| (*f, b.get("bb0").cloned().unwrap_or_else(|| b.values().flatten().copied().collect())))
            .collect();
        rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
        let _ = writeln!(out, "| function | kinds at entry | which |\n|---|---:|---|");
        for (f, kinds) in rows.iter().take(40) {
            let which: Vec<String> = kinds.iter().map(|k| name("ty-kind", *k)).collect();
            let _ = writeln!(out, "| `{f}` | {} | {} |", kinds.len(), which.join(" "));
        }
        out.push('\n');
    }

    // 5. Call pairs.
    if !pairs.is_empty() {
        let named: Vec<(&String, &String)> = pairs.iter().filter_map(|(a, b)| Some((t.functions.get(a)?, t.functions.get(b)?))).collect();
        let _ = writeln!(out, "## Call pairs\n");
        let _ = writeln!(
            out,
            "{} distinct (caller, callee) pairs of instrumented functions ({} with both named; a caller of 0 is a thread's first function).\n",
            pairs.len(),
            named.len()
        );
        if let Some(graph) = &args.graph {
            // The call graph's direct and resolved edges between bodies that both ran.
            let mut hash_of: HashMap<String, String> = HashMap::new();
            let mut edges: HashSet<(String, String)> = HashSet::new();
            for file in mirth_lab::coverage::files_with(graph, "graph") {
                for line in String::from_utf8_lossy(&std::fs::read(&file).unwrap_or_default()).lines() {
                    let f: Vec<&str> = line.split('\t').collect();
                    match f.first() {
                        Some(&"body") if f.len() > 2 => {
                            hash_of.insert(f[2].to_owned(), f[1].to_owned());
                        }
                        Some(&"edge") if f.len() > 3 && (f[3] == "call" || f[3] == "resolved") => {
                            edges.insert((f[1].to_owned(), f[2].to_owned()));
                        }
                        _ => {}
                    }
                }
            }
            let ran_hashes: HashSet<&String> = ran.iter().filter_map(|p| hash_of.get(*p)).collect();
            let between_ran: HashSet<&(String, String)> = edges.iter().filter(|(a, b)| ran_hashes.contains(a) && ran_hashes.contains(b)).collect();
            let taken: HashSet<(String, String)> =
                named.iter().filter_map(|(a, b)| Some((hash_of.get(*a)?.clone(), hash_of.get(*b)?.clone()))).collect();
            let covered = between_ran.iter().filter(|e| taken.contains(**e)).count();
            let outside = taken.iter().filter(|e| !edges.contains(*e)).count();
            let _ = writeln!(
                out,
                "Of the call graph's {} direct and resolved edges between functions that both ran, {} were taken as call pairs ({:.1}%). {} pairs are not such edges (calls through function pointers, `dyn`, closures passed through code outside the compiler, or the caller left stale by unwinding).\n",
                between_ran.len(),
                covered,
                100.0 * covered as f64 / between_ran.len().max(1) as f64,
                outside
            );
            // Per callee crate.
            let mut per: BTreeMap<String, (usize, usize)> = BTreeMap::new();
            let path_of_hash: HashMap<&String, &String> = hash_of.iter().map(|(p, h)| (h, p)).collect();
            for e in &between_ran {
                let krate = path_of_hash.get(&e.1).map(|p| p.split("::").next().unwrap_or("").to_owned()).unwrap_or_default();
                let row = per.entry(krate).or_default();
                row.0 += 1;
                row.1 += taken.contains(*e) as usize;
            }
            let mut rows: Vec<(&String, &(usize, usize))> = per.iter().filter(|(_, r)| r.0 >= 200).collect();
            rows.sort_by(|a, b| (a.1.1 as f64 / a.1.0 as f64).partial_cmp(&(b.1.1 as f64 / b.1.0 as f64)).unwrap());
            let _ = writeln!(out, "| callee crate (200+ edges) | edges between functions that ran | taken | % |\n|---|---:|---:|---:|");
            for (k, (n, c)) in rows.iter().take(30) {
                let _ = writeln!(out, "| {k} | {n} | {c} | {:.1} |", 100.0 * *c as f64 / *n as f64);
            }
            out.push('\n');
        }
    }

    // 6. MIR pass effect.
    if !passes.is_empty() {
        let mut by_pass: BTreeMap<&str, (BTreeSet<&str>, BTreeSet<&str>, BTreeSet<&str>)> = BTreeMap::new(); // (kinds changed, kinds unchanged only..., levels)
        for (pass, kind, level, changed) in &passes {
            let row = by_pass.entry(pass.as_str()).or_default();
            if *changed {
                row.0.insert(kind.as_str());
            } else {
                row.1.insert(kind.as_str());
            }
            row.2.insert(level.as_str());
        }
        let never: Vec<&str> = by_pass.iter().filter(|(_, r)| r.0.is_empty()).map(|(p, _)| *p).collect();
        let _ = writeln!(out, "## MIR pass effect\n");
        let _ = writeln!(
            out,
            "{} passes ran; {} never changed a body in these suites (at any mir-opt-level seen): {}\n",
            by_pass.len(),
            never.len(),
            never.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ")
        );
        let _ = writeln!(out, "| pass | changed bodies of kind | ran on, never changed | mir-opt-levels |\n|---|---|---|---|");
        for (pass, (changed, unchanged, levels)) in &by_pass {
            let only_unchanged: Vec<&&str> = unchanged.iter().filter(|k| !changed.contains(*k)).collect();
            let _ = writeln!(
                out,
                "| `{pass}` | {} | {} | {} |",
                changed.iter().copied().collect::<Vec<_>>().join(" "),
                only_unchanged.iter().map(|k| **k).collect::<Vec<_>>().join(" "),
                levels.iter().copied().collect::<Vec<_>>().join(" ")
            );
        }
        out.push('\n');
    }

    // 7. Lock contention.
    if !locks.is_empty() {
        let mut sites: BTreeMap<String, usize> = BTreeMap::new();
        for l in &locks {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() >= 2 {
                *sites.entry(format!("{}:{}", f[0], f[1])).or_default() += 1;
            }
        }
        let _ = writeln!(out, "## Lock contention\n");
        let _ = writeln!(out, "{} `Lock::lock` call sites were found held at least once under `-Zthreads`:\n", sites.len());
        for site in sites.keys() {
            let _ = writeln!(out, "- `{site}`");
        }
        out.push('\n');
    }

    std::fs::write(&args.out, &out)?;
    println!("written {}", args.out.display());
    Ok(ExitCode::SUCCESS)
}

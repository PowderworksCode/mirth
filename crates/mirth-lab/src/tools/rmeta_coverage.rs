//! Metadata-table coverage (docs/coverage-plan.md, rank 8): which of `define_tables!`'s tables the
//! blessed metadata records show encoded, and decoded, per crate type.
//!
//! Encoded: the `encoded` section of each process in `tests/rmeta/*.txt` names the table each write
//! site fills (`self.tables.<table>[…] <- …`). Decoded: the `read through queries` section names
//! the extern queries a process ran against its dependencies; `cstore_impl.rs`'s `provide!` says
//! which tables each reads (`{ table }` forms directly, other providers through the decoder
//! methods they call, followed through `decoder.rs`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::sitemap::home;
use regex::Regex;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The blessed records (default: tests/rmeta/*.txt in this checkout).
    #[arg(long)]
    records: Vec<PathBuf>,
    #[arg(long, default_value_os_t = home("mirth-work/rust"))]
    rust: PathBuf,
    #[arg(long)]
    out: Option<PathBuf>,
}

static TABLES_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"tables\s*\.\s*(\w+)").unwrap());

/// `name => { body }` entries of a macro body, braces matched.
fn entries(body: &str) -> Vec<(String, String)> {
    static HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*(\w+)\s*=>\s*\{").unwrap());
    let mut out = Vec::new();
    for m in HEAD.captures_iter(body) {
        let start = m.get(0).unwrap().end();
        let mut depth = 1;
        let mut end = start;
        for (i, ch) in body[start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push((m[1].to_owned(), body[start..end].to_owned()));
    }
    out
}

/// Tables each function of `decoder.rs` reads, directly or through the methods it calls.
fn method_tables(src: &str) -> HashMap<String, BTreeSet<String>> {
    static FN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bfn\s+(\w+)").unwrap());
    static CALL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.\s*(\w+)\s*(?:::<[^>]*>)?\s*\(").unwrap());
    let starts: Vec<(usize, String)> = FN.captures_iter(src).map(|c| (c.get(0).unwrap().start(), c[1].to_owned())).collect();
    let mut direct: HashMap<String, (BTreeSet<String>, BTreeSet<String>)> = HashMap::new();
    for (k, (at, name)) in starts.iter().enumerate() {
        let end = starts.get(k + 1).map_or(src.len(), |n| n.0);
        let body = &src[*at..end];
        let e = direct.entry(name.clone()).or_default();
        e.0.extend(TABLES_REF.captures_iter(body).map(|c| c[1].to_owned()));
        e.1.extend(CALL.captures_iter(body).map(|c| c[1].to_owned()));
    }
    let mut out: HashMap<String, BTreeSet<String>> = HashMap::new();
    for name in direct.keys() {
        let mut seen = BTreeSet::new();
        let mut stack = vec![name.clone()];
        let mut tables = BTreeSet::new();
        while let Some(f) = stack.pop() {
            if !seen.insert(f.clone()) {
                continue;
            }
            if let Some((t, calls)) = direct.get(&f) {
                tables.extend(t.iter().cloned());
                stack.extend(calls.iter().filter(|c| direct.contains_key(*c)).cloned());
            }
        }
        out.insert(name.clone(), tables);
    }
    out
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let rmeta = args.rust.join("compiler/rustc_metadata/src/rmeta");
    let tables: Vec<(String, bool)> = super::coverage_static::rmeta_tables(&std::fs::read_to_string(rmeta.join("mod.rs"))?);
    let names: BTreeSet<&str> = tables.iter().map(|t| t.0.as_str()).collect();
    let cstore = std::fs::read_to_string(rmeta.join("decoder/cstore_impl.rs"))?;
    let methods = method_tables(&std::fs::read_to_string(rmeta.join("decoder.rs"))?);
    let provide = cstore.find("provide! {").map(|i| &cstore[i..]).unwrap_or("");
    let provide = &provide[..provide.find("\n}\n").unwrap_or(provide.len())];
    static CDATA_CALL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"cdata\s*\.\s*(\w+)\s*\(").unwrap());
    let mut query_tables: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (q, body) in entries(provide) {
        let t = body.trim();
        let mut set: BTreeSet<String> = BTreeSet::new();
        if t == "table" || t == "table_defaulted_array" || t == "table_direct" {
            set.insert(q.clone());
        }
        set.extend(TABLES_REF.captures_iter(&body).map(|c| c[1].to_owned()));
        for c in CDATA_CALL.captures_iter(&body) {
            set.extend(methods.get(&c[1]).into_iter().flatten().cloned());
        }
        set.retain(|t| names.contains(t.as_str()));
        query_tables.insert(q, set);
    }

    let records = if args.records.is_empty() {
        mirth_lab::coverage::files_with(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/rmeta"), "txt")
    } else {
        args.records.clone()
    };
    static HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^== (\S+) \(([\w-]+)\)").unwrap());
    static READ: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+\d+\s+\d+ items.*\s(\w+)\s*$").unwrap());
    // table -> crate type -> (encoded, decoded)
    let mut seen: BTreeMap<String, BTreeMap<String, (bool, bool)>> = BTreeMap::new();
    let mut types: BTreeSet<String> = BTreeSet::new();
    let mut unmapped: BTreeSet<String> = BTreeSet::new();
    for path in &records {
        let text = std::fs::read_to_string(path)?;
        let (mut ctype, mut section) = (String::new(), "");
        for line in text.lines() {
            if let Some(c) = HEADER.captures(line) {
                ctype = c[2].to_owned();
                types.insert(ctype.clone());
                continue;
            }
            if !line.starts_with(' ') {
                section = if line.starts_with("encoded") { "enc" } else if line.starts_with("read through queries") { "read" } else { "" };
                continue;
            }
            match section {
                "enc" => {
                    for c in TABLES_REF.captures_iter(line) {
                        if names.contains(&c[1]) {
                            seen.entry(c[1].to_owned()).or_default().entry(ctype.clone()).or_default().0 = true;
                        }
                    }
                }
                "read" => {
                    if let Some(c) = READ.captures(line) {
                        match query_tables.get(&c[1]) {
                            Some(ts) => {
                                for t in ts {
                                    seen.entry(t.clone()).or_default().entry(ctype.clone()).or_default().1 = true;
                                }
                            }
                            None => {
                                unmapped.insert(c[1].to_owned());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let enc_any = tables.iter().filter(|t| seen.get(&t.0).is_some_and(|m| m.values().any(|v| v.0))).count();
    let dec_any = tables.iter().filter(|t| seen.get(&t.0).is_some_and(|m| m.values().any(|v| v.1))).count();
    let mut out = String::new();
    let _ = writeln!(out, "{} records; crate types in them: {:?} (dylib, staticlib, cdylib and rlib-only builds appear in none)", records.len(), types);
    let _ = writeln!(out, "metadata tables: {}; encoded in some record: {enc_any}; decoded through an extern query in some record: {dec_any}", tables.len());
    for ty in &types {
        let e = tables.iter().filter(|t| seen.get(&t.0).and_then(|m| m.get(ty)).is_some_and(|v| v.0)).count();
        let d = tables.iter().filter(|t| seen.get(&t.0).and_then(|m| m.get(ty)).is_some_and(|v| v.1)).count();
        let _ = writeln!(out, "  {ty:11} encoded {e:3}  decoded {d:3}");
    }
    let no_reader = tables.iter().filter(|t| !query_tables.values().any(|s| s.contains(&t.0))).count();
    let _ = writeln!(out, "  tables no extern query reads (read by the decoder directly, or at crate load): {no_reader}");
    if !unmapped.is_empty() {
        let _ = writeln!(out, "  queries read in the records with no provider entry found: {}", unmapped.len());
    }
    print!("{out}");
    if let Some(dir) = &args.out {
        std::fs::create_dir_all(dir)?;
        let mut text = String::from("# Metadata tables\n\n| table | kind | ");
        for ty in &types {
            let _ = write!(text, "{ty} enc | {ty} dec | ");
        }
        text.push_str("read by queries |\n|---|---|");
        for _ in &types {
            text.push_str("---|---|");
        }
        text.push_str("---|\n");
        for (t, defaulted) in &tables {
            let _ = write!(text, "| {t} | {} | ", if *defaulted { "defaulted" } else { "optional" });
            for ty in &types {
                let v = seen.get(t).and_then(|m| m.get(ty)).copied().unwrap_or_default();
                let _ = write!(text, "{} | {} | ", if v.0 { "yes" } else { "" }, if v.1 { "yes" } else { "" });
            }
            let readers: Vec<&String> = query_tables.iter().filter(|(_, s)| s.contains(t)).map(|(q, _)| q).collect::<BTreeSet<_>>().into_iter().collect();
            let _ = writeln!(text, "{} |", readers.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        }
        std::fs::write(dir.join("gaps-rmeta.md"), text)?;
    }
    Ok(ExitCode::SUCCESS)
}

//! Which parts of the Rust grammar a fixture's sources use, by Ur's grammar.
//!
//! Ur's Rust grammar (Urscal modules, `syntax Sort = Label: ... | Label: ... | Other ;`) names
//! each alternative of each syntax sort. `ur parse --tree` prints a file's tree with every node as
//! `(Sort::Label ...)` and every literal token in quotes. Two measures:
//!
//!   alternatives  each labeled alternative, by construct: the label within its module, with
//!                 Conditions.rsc (expressions in condition position) counted as Expressions
//!   literals      each keyword and operator the syntax rules mention, used or not
//!
//! Prints what is missing, by module, and a summary. Lexical rules, layout and keyword lists are
//! not syntax alternatives and are left out.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;

use mirth_lab::coverage::to_json_indent;
use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    ur: PathBuf,
    /// Ur's Rust grammar: <ur>/ecosystems/rust/language.
    #[arg(long)]
    grammar: PathBuf,
    fixture: PathBuf,
    #[arg(long)]
    json: Option<PathBuf>,
}

const STRING: &str = r#""(?:[^"\\]|\\.)*""#;
static STRING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(STRING).unwrap());
static STRING_AT: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!("^{STRING}")).unwrap());
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"//[^\n]*").unwrap());
static SYNTAX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^syntax\s+(\w+)[^=]*=").unwrap());
static ATTR_STRING: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"@\w+={STRING}")).unwrap());
static ATTR_EMPTY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"@\w+="[^"]*""#).unwrap());
static NODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\((\w+)::(\w+)").unwrap());

/// Conditions.rsc repeats the expression sorts for condition position (no struct literals): the
/// same constructs, so counted with the expressions.
fn family(stem: &str) -> &str {
    if stem == "Conditions" { "Expressions" } else { stem }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the character before byte `i` is neither a word character nor `:`.
fn free_before(text: &str, i: usize) -> bool {
    text[..i].chars().next_back().is_none_or(|c| !is_word(c) && c != ':')
}

/// (sort, body) for each `syntax` rule of a module: from `syntax Name ... =` to the `;` that
/// ends it, outside quotes and brackets.
fn syntax_rules(text: &str) -> Vec<(String, String)> {
    let bytes = text.as_bytes();
    SYNTAX
        .captures_iter(text)
        .map(|m| {
            let start = m.get(0).unwrap().end();
            let (mut i, mut depth) = (start, 0i32);
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => {
                        i += STRING_AT.find(&text[i..]).map_or(1, |q| q.end());
                        continue;
                    }
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    b';' if depth == 0 => break,
                    _ => {}
                }
                i += 1;
            }
            (m[1].to_owned(), text[start..i.min(bytes.len())].to_owned())
        })
        .collect()
}

/// The labels of a rule body: `Label:` (not `Label::`) not preceded by a word character or `:`.
fn labels(body: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let c = body[i..].chars().next().unwrap();
        if c.is_ascii_uppercase() && free_before(body, i) {
            let end = body[i..].find(|c: char| !is_word(c)).map_or(body.len(), |e| i + e);
            let colon = body[end..].find(|c: char| !c.is_whitespace()).map_or(body.len(), |e| end + e);
            if body[colon..].starts_with(':') && !body[colon + 1..].starts_with(':') {
                out.push(&body[i..end]);
                i = colon + 1;
                continue;
            }
        }
        i += c.len_utf8();
    }
    out
}

/// The quoted tokens of a tree: a string not preceded by a word character or `:`.
fn quoted(tree: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(q) = tree[i..].find('"').map(|q| i + q) {
        if free_before(tree, q)
            && let Some(m) = STRING_AT.find(&tree[q..])
        {
            out.push(&tree[q + 1..q + m.end() - 1]);
            i = q + m.end();
        } else {
            i = q + 1;
        }
    }
    out
}

/// Python's repr of a string.
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::from(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

fn modules(grammar: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = mirth_lab::coverage::files_with(grammar, "rsc");
    files.sort();
    files
}

#[derive(Serialize)]
struct Report {
    alternatives: BTreeMap<String, usize>,
    literals: BTreeMap<String, usize>,
    failed: Vec<String>,
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let mut grammar: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut literals: HashMap<String, String> = HashMap::new();
    let mut sort_family: HashMap<String, String> = HashMap::new();
    for f in modules(&args.grammar) {
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned();
        let fam = family(&stem).to_owned();
        let text = COMMENT.replace_all(&std::fs::read_to_string(&f)?, "").into_owned();
        let rules = syntax_rules(&text);
        for (sort, _) in &rules {
            sort_family.entry(sort.clone()).or_insert_with(|| fam.clone());
        }
        if ["Testing", "Semantics", "Language", "Rust"].contains(&stem.as_str()) {
            continue;
        }
        for (sort, body) in rules {
            let body = ATTR_STRING.replace_all(&body, "");
            for lit in STRING_RE.find_iter(&body) {
                let lit = &lit.as_str()[1..lit.as_str().len() - 1];
                let lit = lit.replace("\\<", "<").replace("\\>", ">").replace("\\\"", "\"").replace("\\\\", "\\");
                literals.entry(lit).or_insert_with(|| fam.clone());
            }
            let bare = STRING_RE.replace_all(&body, "\"\"");
            let bare = ATTR_EMPTY.replace_all(&bare, "");
            for label in labels(&bare) {
                grammar.entry((fam.clone(), label.to_owned())).or_default().push(sort.clone());
            }
        }
    }
    let mut files: Vec<PathBuf> = WalkDir::new(&args.fixture)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs") && !p.components().any(|c| c.as_os_str() == "target"))
        .collect();
    files.sort();
    let mut used_alts: HashMap<(String, String), usize> = HashMap::new();
    let mut used_lits: HashMap<String, usize> = HashMap::new();
    let mut failed = Vec::new();
    for f in &files {
        let out = Command::new(&args.ur).args(["parse", "--tree"]).arg(f).output()?;
        let tree = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() || !tree.contains("(File::") {
            failed.push(f.display().to_string());
            continue;
        }
        for c in NODE.captures_iter(&tree) {
            let fam = sort_family.get(&c[1]).cloned().unwrap_or_else(|| c[1].to_owned());
            *used_alts.entry((fam, c[2].to_owned())).or_default() += 1;
        }
        for lit in quoted(&tree) {
            *used_lits.entry(lit.replace("\\\"", "\"").replace("\\\\", "\\")).or_default() += 1;
        }
    }
    let mut by_module: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut missing_alts = 0;
    for ((fam, label), sorts) in &grammar {
        if !used_alts.contains_key(&(fam.clone(), label.clone())) {
            let sorts: BTreeSet<&String> = sorts.iter().collect();
            by_module.entry(fam).or_default().push(format!("{label} ({})", sorts.into_iter().cloned().collect::<Vec<_>>().join("/")));
            missing_alts += 1;
        }
    }
    let mut missing_lits: Vec<(&String, &String)> = literals.iter().filter(|(l, _)| !used_lits.contains_key(*l)).map(|(l, m)| (m, l)).collect();
    missing_lits.sort();
    let mut lit_by_module: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (m, lit) in &missing_lits {
        lit_by_module.entry(m).or_default().push(py_repr(lit));
    }
    let names: BTreeSet<&str> = by_module.keys().chain(lit_by_module.keys()).copied().collect();
    for m in names {
        println!("== {m}");
        if let Some(a) = by_module.get(m) {
            println!("  alternatives: {}", a.join(", "));
        }
        if let Some(l) = lit_by_module.get(m) {
            println!("  literals: {}", l.join(" "));
        }
    }
    let failed_list = if failed.is_empty() {
        String::new()
    } else {
        format!(": [{}]", failed.iter().map(|f| py_repr(f)).collect::<Vec<_>>().join(", "))
    };
    println!("{} files, {} failed to parse{failed_list}", files.len(), failed.len());
    println!("alternatives: {} of {} used", grammar.len() - missing_alts, grammar.len());
    println!("literals: {} of {} used", literals.len() - missing_lits.len(), literals.len());
    if let Some(j) = &args.json {
        let report = Report {
            alternatives: grammar.keys().map(|(f, l)| (format!("{f}::{l}"), used_alts.get(&(f.clone(), l.clone())).copied().unwrap_or(0))).collect(),
            literals: literals.keys().map(|l| (l.clone(), used_lits.get(l).copied().unwrap_or(0))).collect(),
            failed,
        };
        std::fs::write(j, to_json_indent(&report, 1))?;
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_quotes() {
        assert_eq!(labels("A: x | Foo::Bar | bC: y | Baz : z | (Q:: r) | Last:"), ["A", "Baz", "Last"]);
        assert_eq!(quoted(r#"(X "a" b"c" "d\"e" :"f")"#), ["a", r#"d\"e"#]);
        assert_eq!(py_repr("it's"), "\"it's\"");
        assert_eq!(py_repr("a\\b"), "'a\\\\b'");
    }
}

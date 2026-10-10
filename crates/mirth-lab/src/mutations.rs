//! Random mechanical edits to Rust source, shared by the fuzzers.
//!
//! Each edit takes the file's text, a random generator and a counter, and returns the new text,
//! or None when it does not apply. EDITS lists them with weights.

use std::sync::LazyLock;

use rand::distr::Distribution;
use rand::distr::weighted::WeightedIndex;
use rand::rngs::StdRng;
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, SeedableRng};
use regex::Regex;

pub type Edit = fn(&str, &mut StdRng, usize) -> Option<String>;

/// A generator seeded by a name (a test path), so a run can be repeated.
pub fn seeded(name: &str) -> StdRng {
    let digest = crate::artifacts::sha256(name.as_bytes());
    StdRng::seed_from_u64(u64::from_str_radix(&digest[..16], 16).unwrap())
}

/// A random edit, by weight.
pub fn pick(rng: &mut StdRng) -> (&'static str, Edit) {
    static WEIGHTS: LazyLock<WeightedIndex<u32>> = LazyLock::new(|| WeightedIndex::new(EDITS.iter().map(|e| e.2)).unwrap());
    let (name, f, _) = EDITS[WEIGHTS.sample(rng)];
    (name, f)
}

static INDENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*").unwrap());

fn indent(line: &str) -> &str {
    INDENT.find(line).map_or("", |m| m.as_str())
}

/// Top-level items: blank-line separated, continuation blocks merged.
pub fn blocks(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in text.split("\n\n") {
        let continues = b.chars().next().is_some_and(char::is_whitespace) || b.starts_with('}') || b.starts_with("where");
        match out.last_mut() {
            Some(last) if continues => {
                last.push_str("\n\n");
                last.push_str(b);
            }
            _ => out.push(b.to_owned()),
        }
    }
    out
}

fn lines(text: &str) -> Vec<String> {
    text.split('\n').map(str::to_owned).collect()
}

pub fn comment_line(text: &str, rng: &mut StdRng, n: usize) -> Option<String> {
    let mut l = lines(text);
    let i = rng.random_range(0..=l.len());
    let ind = indent(l.get(i).map_or("", String::as_str)).to_owned();
    l.insert(i, format!("{ind}// fuzz {n}"));
    Some(l.join("\n"))
}

pub fn blank_line(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let mut l = lines(text);
    let i = rng.random_range(0..=l.len());
    l.insert(i, String::new());
    Some(l.join("\n"))
}

pub fn remove_comment(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let mut l = lines(text);
    let idx: Vec<usize> = (0..l.len())
        .filter(|&i| {
            let t = l[i].trim();
            t.starts_with("//") && !t.starts_with("//!")
        })
        .collect();
    let i = *idx.choose(rng)?;
    l.remove(i);
    Some(l.join("\n"))
}

pub fn indent_line(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let mut l = lines(text);
    let idx: Vec<usize> = (0..l.len()).filter(|&i| !l[i].trim().is_empty()).collect();
    let i = *idx.choose(rng)?;
    l[i] = format!("    {}", l[i]);
    Some(l.join("\n"))
}

pub fn swap_items(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    let i = rng.random_range(1..b.len() - 1);
    b.swap(i, i + 1);
    Some(b.join("\n\n"))
}

pub fn move_item_to_end(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    let item = b.remove(rng.random_range(1..b.len()));
    b.push(item.trim_end_matches('\n').to_owned());
    Some(b.join("\n\n") + "\n")
}

pub fn delete_item(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    b.remove(rng.random_range(1..b.len()));
    Some(b.join("\n\n"))
}

static FN_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^(pub(\([^)]*\))? )?(const )?(async )?fn \w+").unwrap());
static FN_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bfn (\w+)").unwrap());

pub fn duplicate_fn(text: &str, rng: &mut StdRng, n: usize) -> Option<String> {
    let mut b = blocks(text);
    let fns: Vec<usize> = (0..b.len()).filter(|&i| FN_ITEM.is_match(&b[i])).collect();
    let i = *fns.choose(rng)?;
    let copy = FN_NAME.replacen(&b[i], 1, format!("fn ${{1}}_fuzz{n}")).into_owned();
    b.insert(i + 1, copy);
    Some(b.join("\n\n"))
}

const ADDITIONS: &[&str] = &[
    "fn fuzz_private_{n}() -> u32 { {n} }",
    "pub fn fuzz_public_{n}(x: u32) -> u32 { x.wrapping_mul({n}) }",
    "#[inline]\npub fn fuzz_inline_{n}<T: Clone>(x: &T) -> (T, u32) { (x.clone(), {n}) }",
    "pub const FUZZ_{n}: &str = \"fuzz {n}\";",
    "pub static FUZZ_STATIC_{n}: [u8; 3] = [{n} as u8, 1, 2];",
    "#[derive(Debug, Clone, PartialEq)]\npub struct Fuzz{n}<T, const N: usize> { pub items: [T; N], pub tag: &'static str }",
    "pub enum FuzzEnum{n} { A(u32), B { x: i64 }, C }",
    "pub trait FuzzTrait{n} { fn go(&self) -> impl Sized; const K: u32 = {n}; }",
    "pub async fn fuzz_async_{n}() -> u32 { {n} }",
    "pub type FuzzAlias{n}<T> = Vec<(T, u32)>;",
    "macro_rules! fuzz_macro_{n} { ($e:expr) => { $e + {n} }; }",
    "pub mod fuzz_mod_{n} { pub fn inner() -> &'static str { \"{n}\" } }",
];

pub fn add_item(text: &str, rng: &mut StdRng, n: usize) -> Option<String> {
    let mut b = blocks(text);
    let i = rng.random_range(1..=b.len());
    b.insert(i, ADDITIONS.choose(rng).unwrap().replace("{n}", &n.to_string()));
    Some(b.join("\n\n"))
}

fn word_or(c: Option<char>, extra: char) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == extra)
}

static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

pub fn int_literal(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    // A whole run of digits, not touching a word character or a dot on either side.
    let ms: Vec<regex::Match> = DIGITS
        .find_iter(text)
        .filter(|m| !word_or(text[..m.start()].chars().next_back(), '.') && !word_or(text[m.end()..].chars().next(), '.'))
        .collect();
    let m = ms.choose(rng)?;
    let bumped = m.as_str().parse::<u128>().map_or_else(|_| format!("{}1", m.as_str()), |v| (v + 1).to_string());
    Some(format!("{}{bumped}{}", &text[..m.start()], &text[m.end()..]))
}

static STRING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"\\\n]*)""#).unwrap());

pub fn str_literal(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    // A string literal not preceded by a word character or a backslash; after a rejected
    // opening quote the search resumes one character later, as a lookbehind would.
    let mut ms = Vec::new();
    let mut pos = 0;
    while let Some(c) = STRING.captures_at(text, pos) {
        let m = c.get(0).unwrap();
        if word_or(text[..m.start()].chars().next_back(), '\\') {
            pos = m.start() + 1;
        } else {
            ms.push((m.start(), m.end(), c[1].to_owned()));
            pos = m.end();
        }
    }
    let (s, e, body) = ms.choose(rng)?;
    Some(format!("{}\"{body}~\"{}", &text[..*s], &text[*e..]))
}

static FN_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(pub(\([^)]*\))? )?(const )?fn ").unwrap());

pub fn toggle_inline(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let mut l = lines(text);
    let inl: Vec<usize> = (0..l.len()).filter(|&i| matches!(l[i].trim(), "#[inline]" | "#[inline(never)]" | "#[inline(always)]")).collect();
    let fns: Vec<usize> = (0..l.len()).filter(|&i| FN_LINE.is_match(&l[i])).collect();
    if !inl.is_empty() && rng.random::<f64>() < 0.5 {
        let i = *inl.choose(rng).unwrap();
        l.remove(i);
    } else if !fns.is_empty() {
        let i = *fns.choose(rng).unwrap();
        let attr = ["#[inline]", "#[inline(never)]", "#[cold]", "#[must_use]"].choose(rng).unwrap();
        let ind = indent(&l[i]).to_owned();
        l.insert(i, format!("{ind}{attr}"));
    } else {
        return None;
    }
    Some(l.join("\n"))
}

static ITEM_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(pub(\([^)]*\))? )?(fn|struct|enum|trait|const|static|type|mod) ").unwrap());

pub fn doc_comment(text: &str, rng: &mut StdRng, n: usize) -> Option<String> {
    let mut l = lines(text);
    let idx: Vec<usize> = (0..l.len()).filter(|&i| ITEM_LINE.is_match(&l[i])).collect();
    let i = *idx.choose(rng)?;
    let ind = indent(&l[i]).to_owned();
    l.insert(i, format!("{ind}/// Fuzz doc {n}, see [`Vec`]."));
    Some(l.join("\n"))
}

static DERIVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[derive\(([^)]*)\)\]").unwrap());

pub fn reorder_derive(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let ms: Vec<regex::Captures> = DERIVE.captures_iter(text).collect();
    let c = ms.choose(rng)?;
    let mut names: Vec<&str> = c[1].split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
    if names.len() < 2 {
        return None;
    }
    names.shuffle(rng);
    let m = c.get(0).unwrap();
    Some(format!("{}#[derive({})]{}", &text[..m.start()], names.join(", "), &text[m.end()..]))
}

static PUB_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bpub (fn|struct|enum|const|static|trait|mod|type) ").unwrap());

pub fn narrow_visibility(text: &str, rng: &mut StdRng, _: usize) -> Option<String> {
    let ms: Vec<regex::Captures> = PUB_ITEM.captures_iter(text).collect();
    let c = ms.choose(rng)?;
    let m = c.get(0).unwrap();
    Some(format!("{}pub(crate) {} {}", &text[..m.start()], &c[1], &text[m.end()..]))
}

static LET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\blet (mut )?([a-z_][a-z0-9_]*)\b").unwrap());

pub fn rename_local(text: &str, rng: &mut StdRng, n: usize) -> Option<String> {
    let ms: Vec<regex::Captures> = LET.captures_iter(text).collect();
    let c = ms.choose(rng)?;
    let name = &c[2];
    if name == "_" {
        return None;
    }
    let m = c.get(0).unwrap();
    // Rename from the binding to the end of the enclosing top-level block.
    let end = text[m.end()..].find("\n}\n").map_or(text.len(), |i| m.end() + i);
    let re = Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();
    let body = re.replace_all(&text[m.start()..end], format!("{name}_f{n}").as_str()).into_owned();
    Some(format!("{}{body}{}", &text[..m.start()], &text[end..]))
}

pub const EDITS: &[(&str, Edit, u32)] = &[
    ("comment_line", comment_line, 10),
    ("blank_line", blank_line, 6),
    ("remove_comment", remove_comment, 3),
    ("indent_line", indent_line, 4),
    ("swap_items", swap_items, 6),
    ("move_item_to_end", move_item_to_end, 3),
    ("delete_item", delete_item, 2),
    ("duplicate_fn", duplicate_fn, 4),
    ("add_item", add_item, 8),
    ("int_literal", int_literal, 6),
    ("str_literal", str_literal, 4),
    ("toggle_inline", toggle_inline, 4),
    ("doc_comment", doc_comment, 4),
    ("reorder_derive", reorder_derive, 2),
    ("narrow_visibility", narrow_visibility, 2),
    ("rename_local", rename_local, 3),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals() {
        let mut rng = seeded("t");
        assert_eq!(int_literal("let x = 12; y.0; a1", &mut rng, 0).unwrap(), "let x = 13; y.0; a1");
        assert_eq!(str_literal(r#"r"a" + "b""#, &mut rng, 0).unwrap(), r#"r"a" + "b~""#);
        assert!(int_literal("x1 y.2", &mut rng, 0).is_none());
    }

    #[test]
    fn blocks_merge_continuations() {
        assert_eq!(blocks("fn a() {\n\n    x\n}\n\nfn b() {}"), vec!["fn a() {\n\n    x\n}", "fn b() {}"]);
    }

    #[test]
    fn rename() {
        let mut rng = seeded("t");
        let t = "fn f() {\n    let x = 1;\n    x + 1\n}\nfn g() { x }\n";
        assert_eq!(rename_local(t, &mut rng, 3).unwrap(), "fn f() {\n    let x_f3 = 1;\n    x_f3 + 1\n}\nfn g() { x }\n");
    }
}

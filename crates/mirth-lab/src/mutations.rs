//! Random mechanical edits to Rust source, shared by the fuzzers.
//!
//! Each edit takes the file's text, a random source and a counter (which names what the edit
//! adds), and returns the new text, or None when it does not apply. [`EDITS`] lists them with
//! weights; [`pick`] draws one.

use std::sync::LazyLock;

use regex::Regex;

/// The random choices an edit makes. Implemented for every `rand::Rng`; the draws mirror
/// Python's `random` (randrange, choice, choices, shuffle) so that a scripted source replays
/// the same edit in both.
pub trait EditRng {
    /// A float in [0, 1).
    fn float(&mut self) -> f64;
    /// An integer in [0, n), n > 0.
    fn below(&mut self, n: usize) -> usize;
}

impl<R: rand::Rng> EditRng for R {
    fn float(&mut self) -> f64 {
        self.random::<f64>()
    }
    fn below(&mut self, n: usize) -> usize {
        self.random_range(0..n)
    }
}

fn choice<'a, T>(rng: &mut dyn EditRng, v: &'a [T]) -> &'a T {
    &v[rng.below(v.len())]
}

/// Fisher-Yates from the end, as Python's `random.shuffle`.
fn shuffle<T>(rng: &mut dyn EditRng, v: &mut [T]) {
    for i in (1..v.len()).rev() {
        let j = rng.below(i + 1);
        v.swap(i, j);
    }
}

pub type EditFn = fn(&str, &mut dyn EditRng, u64) -> Option<String>;

pub struct Edit {
    pub name: &'static str,
    pub weight: u32,
    pub apply: EditFn,
}

pub const EDITS: &[Edit] = &[
    Edit { name: "comment_line", weight: 10, apply: comment_line },
    Edit { name: "blank_line", weight: 6, apply: blank_line },
    Edit { name: "remove_comment", weight: 3, apply: remove_comment },
    Edit { name: "indent_line", weight: 4, apply: indent_line },
    Edit { name: "swap_items", weight: 6, apply: swap_items },
    Edit { name: "move_item_to_end", weight: 3, apply: move_item_to_end },
    Edit { name: "delete_item", weight: 2, apply: delete_item },
    Edit { name: "duplicate_fn", weight: 4, apply: duplicate_fn },
    Edit { name: "add_item", weight: 8, apply: add_item },
    Edit { name: "int_literal", weight: 6, apply: int_literal },
    Edit { name: "str_literal", weight: 4, apply: str_literal },
    Edit { name: "toggle_inline", weight: 4, apply: toggle_inline },
    Edit { name: "doc_comment", weight: 4, apply: doc_comment },
    Edit { name: "reorder_derive", weight: 2, apply: reorder_derive },
    Edit { name: "narrow_visibility", weight: 2, apply: narrow_visibility },
    Edit { name: "rename_local", weight: 3, apply: rename_local },
];

/// An edit drawn by weight (Python's `random.choices`).
pub fn pick(rng: &mut dyn EditRng) -> &'static Edit {
    let total: u32 = EDITS.iter().map(|e| e.weight).sum();
    let u = rng.float() * total as f64;
    let mut acc = 0.0;
    for e in EDITS {
        acc += e.weight as f64;
        if u < acc {
            return e;
        }
    }
    &EDITS[EDITS.len() - 1]
}

/// The edits that change literals: not for build scripts (Cargo keeps stale OUT_DIR files, so
/// renaming a generated file splits the builds, and a changed number can make it loop).
pub fn changes_literals(e: &Edit) -> bool {
    e.name == "int_literal" || e.name == "str_literal"
}

/// Top-level items: blank-line separated, continuation blocks merged.
fn blocks(text: &str) -> Vec<String> {
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

fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn comment_line(text: &str, rng: &mut dyn EditRng, n: u64) -> Option<String> {
    let mut l = lines(text);
    let i = rng.below(l.len() + 1);
    let indent = l.get(i).map_or("", |s| indent_of(s)).to_owned();
    l.insert(i, format!("{indent}// fuzz {n}"));
    Some(l.join("\n"))
}

fn blank_line(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let mut l = lines(text);
    let i = rng.below(l.len() + 1);
    l.insert(i, String::new());
    Some(l.join("\n"))
}

fn remove_comment(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let mut l = lines(text);
    let idx: Vec<usize> =
        (0..l.len()).filter(|&i| l[i].trim().starts_with("//") && !l[i].trim().starts_with("//!")).collect();
    if idx.is_empty() {
        return None;
    }
    l.remove(*choice(rng, &idx));
    Some(l.join("\n"))
}

fn indent_line(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let mut l = lines(text);
    let idx: Vec<usize> = (0..l.len()).filter(|&i| !l[i].trim().is_empty()).collect();
    if idx.is_empty() {
        return None;
    }
    let i = *choice(rng, &idx);
    l[i] = format!("    {}", l[i]);
    Some(l.join("\n"))
}

fn swap_items(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    let i = 1 + rng.below(b.len() - 2);
    b.swap(i, i + 1);
    Some(b.join("\n\n"))
}

fn move_item_to_end(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    let i = 1 + rng.below(b.len() - 1);
    let item = b.remove(i);
    b.push(item.trim_end_matches('\n').to_owned());
    Some(b.join("\n\n") + "\n")
}

fn delete_item(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    b.remove(1 + rng.below(b.len() - 1));
    Some(b.join("\n\n"))
}

static FN_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(pub(\([^)]*\))? )?(const )?(async )?fn \w+").unwrap());
static FN_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bfn (\w+)").unwrap());

fn duplicate_fn(text: &str, rng: &mut dyn EditRng, n: u64) -> Option<String> {
    let mut b = blocks(text);
    let fns: Vec<usize> = (0..b.len()).filter(|&i| FN_ITEM.is_match(&b[i])).collect();
    if fns.is_empty() {
        return None;
    }
    let i = *choice(rng, &fns);
    let copy = FN_NAME.replacen(&b[i], 1, |c: &regex::Captures| format!("fn {}_fuzz{n}", &c[1])).into_owned();
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

fn add_item(text: &str, rng: &mut dyn EditRng, n: u64) -> Option<String> {
    let mut b = blocks(text);
    let i = 1 + rng.below(b.len());
    b.insert(i, choice(rng, ADDITIONS).replace("{n}", &n.to_string()));
    Some(b.join("\n\n"))
}

/// Python's `\w`: alphanumeric or underscore.
fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

/// One more than a decimal number written in ASCII or other digits, as Python's `int(s) + 1`.
fn increment(s: &str) -> Option<String> {
    let mut digits: Vec<u8> = s.chars().map(|c| c.to_digit(10).map(|d| d as u8)).collect::<Option<_>>()?;
    let mut i = digits.len();
    loop {
        if i == 0 {
            digits.insert(0, 1);
            break;
        }
        i -= 1;
        if digits[i] == 9 {
            digits[i] = 0;
        } else {
            digits[i] += 1;
            break;
        }
    }
    let first = digits.iter().position(|&d| d != 0).unwrap_or(digits.len() - 1);
    Some(digits[first..].iter().map(|d| (b'0' + d) as char).collect())
}

/// Numbers not part of a word or a float: `(?<![\w.])(\d+)(?![\w.])`.
fn int_literal(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let ms: Vec<regex::Match> = DIGITS
        .find_iter(text)
        .filter(|m| {
            let before = text[..m.start()].chars().next_back();
            let after = text[m.end()..].chars().next();
            !before.is_some_and(|c| word(c) || c == '.') && !after.is_some_and(|c| word(c) || c == '.')
        })
        .collect();
    if ms.is_empty() {
        return None;
    }
    let m = choice(rng, &ms);
    Some(format!("{}{}{}", &text[..m.start()], increment(m.as_str())?, &text[m.end()..]))
}

/// Plain string literals: `(?<![\w\\])"([^"\\\n]*)"`, scanned as Python's regex scans.
fn str_literals(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = text[i..].find('"') {
        let start = i + off;
        let before = text[..start].chars().next_back();
        if !before.is_some_and(|c| word(c) || c == '\\') {
            let body = &text[start + 1..];
            let stop = body.find(['"', '\\', '\n']);
            if let Some(s) = stop
                && body.as_bytes()[s] == b'"'
            {
                let end = start + 1 + s + 1;
                out.push((start, end));
                i = end;
                continue;
            }
        }
        i = start + 1;
    }
    out
}

fn str_literal(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let ms = str_literals(text);
    if ms.is_empty() {
        return None;
    }
    let (s, e) = *choice(rng, &ms);
    Some(format!("{}\"{}~\"{}", &text[..s], &text[s + 1..e - 1], &text[e..]))
}

static FN_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(pub(\([^)]*\))? )?(const )?fn ").unwrap());

fn toggle_inline(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let mut l = lines(text);
    let inl: Vec<usize> =
        (0..l.len()).filter(|&i| matches!(l[i].trim(), "#[inline]" | "#[inline(never)]" | "#[inline(always)]")).collect();
    let fns: Vec<usize> = (0..l.len()).filter(|&i| FN_LINE.is_match(&l[i])).collect();
    if !inl.is_empty() && rng.float() < 0.5 {
        l.remove(*choice(rng, &inl));
    } else if !fns.is_empty() {
        let i = *choice(rng, &fns);
        let indent = indent_of(&l[i]).to_owned();
        let attr = choice(rng, &["#[inline]", "#[inline(never)]", "#[cold]", "#[must_use]"]);
        l.insert(i, format!("{indent}{attr}"));
    } else {
        return None;
    }
    Some(l.join("\n"))
}

static ITEM_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(pub(\([^)]*\))? )?(fn|struct|enum|trait|const|static|type|mod) ").unwrap());

fn doc_comment(text: &str, rng: &mut dyn EditRng, n: u64) -> Option<String> {
    let mut l = lines(text);
    let idx: Vec<usize> = (0..l.len()).filter(|&i| ITEM_LINE.is_match(&l[i])).collect();
    if idx.is_empty() {
        return None;
    }
    let i = *choice(rng, &idx);
    let indent = indent_of(&l[i]).to_owned();
    l.insert(i, format!("{indent}/// Fuzz doc {n}, see [`Vec`]."));
    Some(l.join("\n"))
}

static DERIVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[derive\(([^)]*)\)\]").unwrap());

fn reorder_derive(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let ms: Vec<regex::Captures> = DERIVE.captures_iter(text).collect();
    if ms.is_empty() {
        return None;
    }
    let m = choice(rng, &ms);
    let mut names: Vec<&str> = m[1].split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
    if names.len() < 2 {
        return None;
    }
    shuffle(rng, &mut names);
    let all = m.get(0).unwrap();
    Some(format!("{}#[derive({})]{}", &text[..all.start()], names.join(", "), &text[all.end()..]))
}

static PUB_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bpub (fn|struct|enum|const|static|trait|mod|type) ").unwrap());

fn narrow_visibility(text: &str, rng: &mut dyn EditRng, _: u64) -> Option<String> {
    let ms: Vec<regex::Captures> = PUB_ITEM.captures_iter(text).collect();
    if ms.is_empty() {
        return None;
    }
    let m = choice(rng, &ms);
    let all = m.get(0).unwrap();
    Some(format!("{}pub(crate) {} {}", &text[..all.start()], &m[1], &text[all.end()..]))
}

static LET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\blet (mut )?([a-z_][a-z0-9_]*)\b").unwrap());

fn rename_local(text: &str, rng: &mut dyn EditRng, n: u64) -> Option<String> {
    let ms: Vec<regex::Captures> = LET.captures_iter(text).collect();
    if ms.is_empty() {
        return None;
    }
    let m = choice(rng, &ms);
    let name = &m[2];
    if name == "_" {
        return None;
    }
    let (start, after) = (m.get(0).unwrap().start(), m.get(0).unwrap().end());
    // Rename from the binding to the end of the enclosing top-level block.
    let end = text[after..].find("\n}\n").map_or(text.len(), |e| after + e);
    let re = Regex::new(&format!(r"\b{name}\b")).unwrap();
    let body = re.replace_all(&text[start..end], format!("{name}_f{n}").as_str());
    Some(format!("{}{}{}", &text[..start], body, &text[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws from a fixed list of floats in [0, 1); `below(n)` is `floor(u * n)`.
    pub struct Scripted(pub Vec<f64>, pub usize);
    impl EditRng for Scripted {
        fn float(&mut self) -> f64 {
            let u = self.0[self.1 % self.0.len()];
            self.1 += 1;
            u
        }
        fn below(&mut self, n: usize) -> usize {
            ((self.float() * n as f64) as usize).min(n - 1)
        }
    }

    #[test]
    fn literals() {
        assert_eq!(increment("009").as_deref(), Some("10"));
        assert_eq!(increment("99").as_deref(), Some("100"));
        let mut r = Scripted(vec![0.0], 0);
        assert_eq!(int_literal("a1 + 2.0 + 3", &mut r, 0).as_deref(), Some("a1 + 2.0 + 4"));
        assert_eq!(str_literal(r#"r"x" + "y""#, &mut r, 0).as_deref(), Some(r#"r"x" + "y~""#));
    }

    /// Cases written by the Python edits with the same scripted draws (MIRTH_MUTATION_CASES:
    /// a JSON list of {edit, text, n, draws, out}).
    #[test]
    fn same_as_python() {
        let Ok(path) = std::env::var("MIRTH_MUTATION_CASES") else { return };
        let cases: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut bad = 0;
        for c in &cases {
            let edit = EDITS.iter().find(|e| e.name == c["edit"]).unwrap();
            let draws: Vec<f64> = c["draws"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
            let got = (edit.apply)(c["text"].as_str().unwrap(), &mut Scripted(draws, 0), c["n"].as_u64().unwrap());
            if got.as_deref() != c["out"].as_str() {
                bad += 1;
                eprintln!("{}: differs", c["edit"]);
            }
        }
        assert_eq!(bad, 0, "of {} cases", cases.len());
        eprintln!("{} cases agree", cases.len());
    }
}

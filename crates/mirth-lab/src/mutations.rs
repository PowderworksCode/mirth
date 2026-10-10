//! Random mechanical edits to Rust source, shared by the fuzzers and flag-walk.
//!
//! Each edit takes the file's text, a random generator and a counter, and returns the new
//! text, or None when it does not apply. EDITS lists them with weights.

use std::sync::LazyLock;

use fastrand::Rng;
use regex::Regex;

pub type Edit = fn(&str, &mut Rng, usize) -> Option<String>;

/// Top-level items: blank-line separated, continuation blocks merged.
fn blocks(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in text.split("\n\n") {
        let cont = b.chars().next().is_some_and(char::is_whitespace) || b.starts_with('}') || b.starts_with("where");
        match out.last_mut() {
            Some(last) if cont => {
                last.push_str("\n\n");
                last.push_str(b);
            }
            _ => out.push(b.to_owned()),
        }
    }
    out
}

fn indent(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn pick<'a, T>(rng: &mut Rng, v: &'a [T]) -> &'a T {
    &v[rng.usize(..v.len())]
}

fn comment_line(text: &str, rng: &mut Rng, n: usize) -> Option<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let i = rng.usize(..=lines.len());
    let ind = lines.get(i).map_or("", |l| indent(l)).to_owned();
    lines.insert(i, format!("{ind}// fuzz {n}"));
    Some(lines.join("\n"))
}

fn blank_line(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let i = rng.usize(..=lines.len());
    lines.insert(i, "");
    Some(lines.join("\n"))
}

fn remove_comment(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let idx: Vec<usize> =
        (0..lines.len()).filter(|&i| lines[i].trim().starts_with("//") && !lines[i].trim().starts_with("//!")).collect();
    if idx.is_empty() {
        return None;
    }
    lines.remove(*pick(rng, &idx));
    Some(lines.join("\n"))
}

fn indent_line(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let idx: Vec<usize> = (0..lines.len()).filter(|&i| !lines[i].trim().is_empty()).collect();
    if idx.is_empty() {
        return None;
    }
    let i = *pick(rng, &idx);
    lines[i] = format!("    {}", lines[i]);
    Some(lines.join("\n"))
}

fn swap_items(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    let i = rng.usize(1..b.len() - 1);
    b.swap(i, i + 1);
    Some(b.join("\n\n"))
}

fn move_item_to_end(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    let item = b.remove(rng.usize(1..b.len()));
    b.push(item.trim_end_matches('\n').to_owned());
    Some(b.join("\n\n") + "\n")
}

fn delete_item(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let mut b = blocks(text);
    if b.len() < 3 {
        return None;
    }
    b.remove(rng.usize(1..b.len()));
    Some(b.join("\n\n"))
}

static FN_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^(pub(\([^)]*\))? )?(const )?(async )?fn \w+").unwrap());
static FN_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bfn (\w+)").unwrap());

fn duplicate_fn(text: &str, rng: &mut Rng, n: usize) -> Option<String> {
    let mut b = blocks(text);
    let fns: Vec<usize> = (0..b.len()).filter(|&i| FN_ITEM.is_match(&b[i])).collect();
    if fns.is_empty() {
        return None;
    }
    let i = *pick(rng, &fns);
    let copy = FN_NAME.replacen(&b[i], 1, |c: &regex::Captures| format!("fn {}_fuzz{n}", &c[1])).into_owned();
    b.insert(i + 1, copy);
    Some(b.join("\n\n"))
}

/// Items to add; `{n}` is the counter.
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

fn add_item(text: &str, rng: &mut Rng, n: usize) -> Option<String> {
    let mut b = blocks(text);
    let i = rng.usize(1..=b.len());
    b.insert(i, pick(rng, ADDITIONS).replace("{n}", &n.to_string()));
    Some(b.join("\n\n"))
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

fn int_literal(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    // A whole number not touching a word character or a dot.
    let ms: Vec<regex::Match> = DIGITS
        .find_iter(text)
        .filter(|m| {
            let before = text[..m.start()].chars().next_back();
            let after = text[m.end()..].chars().next();
            !is_word(before) && before != Some('.') && !is_word(after) && after != Some('.')
        })
        .collect();
    if ms.is_empty() {
        return None;
    }
    let m = pick(rng, &ms);
    let v: u128 = m.as_str().parse().ok()?;
    Some(format!("{}{}{}", &text[..m.start()], v + 1, &text[m.end()..]))
}

fn str_literal(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    // `"…"` with no quote, backslash or newline inside, not after a word character or a backslash.
    let bytes = text.as_bytes();
    let mut ms: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let before = text[..i].chars().next_back();
            if !is_word(before) && before != Some('\\') {
                let rest = &bytes[i + 1..];
                if let Some(j) = rest.iter().position(|&c| c == b'"' || c == b'\\' || c == b'\n') {
                    if rest[j] == b'"' {
                        ms.push((i, i + 1 + j + 1));
                        i = i + 1 + j + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    if ms.is_empty() {
        return None;
    }
    let (s, e) = *pick(rng, &ms);
    Some(format!("{}\"{}~\"{}", &text[..s], &text[s + 1..e - 1], &text[e..]))
}

static FN_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(pub(\([^)]*\))? )?(const )?fn ").unwrap());
static ITEM_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(pub(\([^)]*\))? )?(fn|struct|enum|trait|const|static|type|mod) ").unwrap());

fn toggle_inline(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let inl: Vec<usize> =
        (0..lines.len()).filter(|&i| ["#[inline]", "#[inline(never)]", "#[inline(always)]"].contains(&lines[i].trim())).collect();
    let fns: Vec<usize> = (0..lines.len()).filter(|&i| FN_LINE.is_match(&lines[i])).collect();
    if !inl.is_empty() && rng.f64() < 0.5 {
        lines.remove(*pick(rng, &inl));
    } else if !fns.is_empty() {
        let i = *pick(rng, &fns);
        let ind = indent(&lines[i]).to_owned();
        let attr = pick(rng, &["#[inline]", "#[inline(never)]", "#[cold]", "#[must_use]"]);
        lines.insert(i, format!("{ind}{attr}"));
    } else {
        return None;
    }
    Some(lines.join("\n"))
}

fn doc_comment(text: &str, rng: &mut Rng, n: usize) -> Option<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let idx: Vec<usize> = (0..lines.len()).filter(|&i| ITEM_LINE.is_match(&lines[i])).collect();
    if idx.is_empty() {
        return None;
    }
    let i = *pick(rng, &idx);
    let ind = indent(&lines[i]).to_owned();
    lines.insert(i, format!("{ind}/// Fuzz doc {n}, see [`Vec`]."));
    Some(lines.join("\n"))
}

static DERIVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[derive\(([^)]*)\)\]").unwrap());

fn reorder_derive(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let ms: Vec<regex::Captures> = DERIVE.captures_iter(text).collect();
    if ms.is_empty() {
        return None;
    }
    let m = pick(rng, &ms);
    let mut names: Vec<&str> = m[1].split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
    if names.len() < 2 {
        return None;
    }
    rng.shuffle(&mut names);
    let all = m.get(0).unwrap();
    Some(format!("{}#[derive({})]{}", &text[..all.start()], names.join(", "), &text[all.end()..]))
}

static PUB_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bpub (fn|struct|enum|const|static|trait|mod|type) ").unwrap());

fn narrow_visibility(text: &str, rng: &mut Rng, _: usize) -> Option<String> {
    let ms: Vec<regex::Captures> = PUB_ITEM.captures_iter(text).collect();
    if ms.is_empty() {
        return None;
    }
    let m = pick(rng, &ms);
    let all = m.get(0).unwrap();
    Some(format!("{}pub(crate) {} {}", &text[..all.start()], &m[1], &text[all.end()..]))
}

static LET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\blet (mut )?([a-z_][a-z0-9_]*)\b").unwrap());

fn rename_local(text: &str, rng: &mut Rng, n: usize) -> Option<String> {
    let ms: Vec<regex::Captures> = LET.captures_iter(text).collect();
    if ms.is_empty() {
        return None;
    }
    let m = pick(rng, &ms);
    let name = &m[2];
    if name == "_" {
        return None;
    }
    let all = m.get(0).unwrap();
    // Rename from the binding to the end of the enclosing top-level block.
    let end = text[all.end()..].find("\n}\n").map_or(text.len(), |i| all.end() + i);
    let word = Regex::new(&format!(r"\b{name}\b")).ok()?;
    let body = word.replace_all(&text[all.start()..end], format!("{name}_f{n}").as_str());
    Some(format!("{}{}{}", &text[..all.start()], body, &text[end..]))
}

pub const EDITS: &[(Edit, u32)] = &[
    (comment_line, 10),
    (blank_line, 6),
    (remove_comment, 3),
    (indent_line, 4),
    (swap_items, 6),
    (move_item_to_end, 3),
    (delete_item, 2),
    (duplicate_fn, 4),
    (add_item, 8),
    (int_literal, 6),
    (str_literal, 4),
    (toggle_inline, 4),
    (doc_comment, 4),
    (reorder_derive, 2),
    (narrow_visibility, 2),
    (rename_local, 3),
];

/// Edits that change a literal (the fuzzers keep them out of build scripts).
pub fn is_literal_edit(e: Edit) -> bool {
    std::ptr::fn_addr_eq(e, int_literal as Edit) || std::ptr::fn_addr_eq(e, str_literal as Edit)
}

/// An edit drawn by weight.
pub fn choose(rng: &mut Rng) -> Edit {
    let total: u32 = EDITS.iter().map(|e| e.1).sum();
    let mut x = rng.u32(..total);
    for (e, w) in EDITS {
        if x < *w {
            return *e;
        }
        x -= w;
    }
    EDITS[0].0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals() {
        let mut rng = Rng::with_seed(1);
        assert_eq!(int_literal("x1 = 2.5 + 7;", &mut rng, 0).as_deref(), Some("x1 = 2.5 + 8;"));
        assert_eq!(str_literal(r#"a"b" + "c""#, &mut rng, 0).as_deref(), Some(r#"a"b" + "c~""#));
    }

    #[test]
    fn blocks_merge_continuations() {
        assert_eq!(blocks("a\n\n  b\n\nc").len(), 2);
    }
}

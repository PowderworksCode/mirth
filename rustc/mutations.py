"""Random mechanical edits to Rust source, shared by fuzz.py and flag-walk.py.

Each edit takes the file's text, a random.Random and a counter, and returns the new text, or
None when it does not apply. EDITS lists them with weights.
"""

import re



def blocks(text):
    """Top-level items: blank-line separated, continuation blocks merged."""
    out = []
    for b in text.split("\n\n"):
        if out and (b[:1].isspace() or b.startswith("}") or b.startswith("where")):
            out[-1] += "\n\n" + b
        else:
            out.append(b)
    return out


def edit_lines(text, rng, f):
    lines = text.split("\n")
    r = f(lines, rng)
    return None if r is None else "\n".join(r)


def comment_line(text, rng, n):
    def f(lines, rng):
        i = rng.randrange(len(lines) + 1)
        indent = re.match(r"\s*", lines[i] if i < len(lines) else "").group(0)
        return lines[:i] + [f"{indent}// fuzz {n}"] + lines[i:]
    return edit_lines(text, rng, f)


def blank_line(text, rng, n):
    return edit_lines(text, rng, lambda lines, rng: (lambda i: lines[:i] + [""] + lines[i:])(rng.randrange(len(lines) + 1)))


def remove_comment(text, rng, n):
    def f(lines, rng):
        idx = [i for i, l in enumerate(lines) if l.strip().startswith("//") and not l.strip().startswith("//!")]
        if not idx:
            return None
        i = rng.choice(idx)
        return lines[:i] + lines[i + 1:]
    return edit_lines(text, rng, f)


def indent_line(text, rng, n):
    def f(lines, rng):
        idx = [i for i, l in enumerate(lines) if l.strip()]
        i = rng.choice(idx)
        lines[i] = "    " + lines[i]
        return lines
    return edit_lines(text, rng, f)


def swap_items(text, rng, n):
    b = blocks(text)
    if len(b) < 3:
        return None
    i = rng.randrange(1, len(b) - 1)
    b[i], b[i + 1] = b[i + 1], b[i]
    return "\n\n".join(b)


def move_item_to_end(text, rng, n):
    b = blocks(text)
    if len(b) < 3:
        return None
    i = rng.randrange(1, len(b))
    item = b.pop(i)
    return "\n\n".join(b + [item.rstrip("\n")]) + "\n"


def delete_item(text, rng, n):
    b = blocks(text)
    if len(b) < 3:
        return None
    b.pop(rng.randrange(1, len(b)))
    return "\n\n".join(b)


def duplicate_fn(text, rng, n):
    b = blocks(text)
    fns = [i for i, x in enumerate(b) if re.search(r"^(pub(\([^)]*\))? )?(const )?(async )?fn \w+", x, re.M)]
    if not fns:
        return None
    i = rng.choice(fns)
    copy = re.sub(r"\bfn (\w+)", lambda m: f"fn {m.group(1)}_fuzz{n}", b[i], count=1)
    b.insert(i + 1, copy)
    return "\n\n".join(b)


ADDITIONS = [
    "fn fuzz_private_{n}() -> u32 {{ {n} }}",
    "pub fn fuzz_public_{n}(x: u32) -> u32 {{ x.wrapping_mul({n}) }}",
    "#[inline]\npub fn fuzz_inline_{n}<T: Clone>(x: &T) -> (T, u32) {{ (x.clone(), {n}) }}",
    "pub const FUZZ_{n}: &str = \"fuzz {n}\";",
    "pub static FUZZ_STATIC_{n}: [u8; 3] = [{n} as u8, 1, 2];",
    "#[derive(Debug, Clone, PartialEq)]\npub struct Fuzz{n}<T, const N: usize> {{ pub items: [T; N], pub tag: &'static str }}",
    "pub enum FuzzEnum{n} {{ A(u32), B {{ x: i64 }}, C }}",
    "pub trait FuzzTrait{n} {{ fn go(&self) -> impl Sized; const K: u32 = {n}; }}",
    "pub async fn fuzz_async_{n}() -> u32 {{ {n} }}",
    "pub type FuzzAlias{n}<T> = Vec<(T, u32)>;",
    "macro_rules! fuzz_macro_{n} {{ ($e:expr) => {{ $e + {n} }}; }}",
    "pub mod fuzz_mod_{n} {{ pub fn inner() -> &'static str {{ \"{n}\" }} }}",
]


def add_item(text, rng, n):
    b = blocks(text)
    i = rng.randrange(1, len(b) + 1)
    b.insert(i, rng.choice(ADDITIONS).format(n=n))
    return "\n\n".join(b)


def int_literal(text, rng, n):
    ms = [m for m in re.finditer(r"(?<![\w.])(\d+)(?![\w.])", text)]
    if not ms:
        return None
    m = rng.choice(ms)
    return text[: m.start()] + str(int(m.group(1)) + 1) + text[m.end():]


def str_literal(text, rng, n):
    ms = [m for m in re.finditer(r'(?<![\w\\])"([^"\\\n]*)"', text)]
    if not ms:
        return None
    m = rng.choice(ms)
    return text[: m.start()] + '"' + m.group(1) + "~" + '"' + text[m.end():]


def toggle_inline(text, rng, n):
    lines = text.split("\n")
    inl = [i for i, l in enumerate(lines) if l.strip() in ("#[inline]", "#[inline(never)]", "#[inline(always)]")]
    fns = [i for i, l in enumerate(lines) if re.match(r"\s*(pub(\([^)]*\))? )?(const )?fn ", l)]
    if inl and rng.random() < 0.5:
        lines.pop(rng.choice(inl))
    elif fns:
        i = rng.choice(fns)
        indent = re.match(r"\s*", lines[i]).group(0)
        lines.insert(i, indent + rng.choice(["#[inline]", "#[inline(never)]", "#[cold]", "#[must_use]"]))
    else:
        return None
    return "\n".join(lines)


def doc_comment(text, rng, n):
    lines = text.split("\n")
    idx = [i for i, l in enumerate(lines) if re.match(r"\s*(pub(\([^)]*\))? )?(fn|struct|enum|trait|const|static|type|mod) ", l)]
    if not idx:
        return None
    i = rng.choice(idx)
    indent = re.match(r"\s*", lines[i]).group(0)
    lines.insert(i, f"{indent}/// Fuzz doc {n}, see [`Vec`].")
    return "\n".join(lines)


def reorder_derive(text, rng, n):
    ms = list(re.finditer(r"#\[derive\(([^)]*)\)\]", text))
    if not ms:
        return None
    m = rng.choice(ms)
    names = [x.strip() for x in m.group(1).split(",") if x.strip()]
    if len(names) < 2:
        return None
    rng.shuffle(names)
    return text[: m.start()] + "#[derive(" + ", ".join(names) + ")]" + text[m.end():]


def narrow_visibility(text, rng, n):
    ms = list(re.finditer(r"\bpub (fn|struct|enum|const|static|trait|mod|type) ", text))
    if not ms:
        return None
    m = rng.choice(ms)
    return text[: m.start()] + "pub(crate) " + m.group(1) + " " + text[m.end():]


def rename_local(text, rng, n):
    ms = list(re.finditer(r"\blet (mut )?([a-z_][a-z0-9_]*)\b", text))
    if not ms:
        return None
    m = rng.choice(ms)
    name = m.group(2)
    if name == "_":
        return None
    # Rename from the binding to the end of the enclosing top-level block.
    end = text.find("\n}\n", m.end())
    end = len(text) if end < 0 else end
    body = re.sub(rf"\b{name}\b", f"{name}_f{n}", text[m.start():end])
    return text[: m.start()] + body + text[end:]


EDITS = [
    (comment_line, 10), (blank_line, 6), (remove_comment, 3), (indent_line, 4),
    (swap_items, 6), (move_item_to_end, 3), (delete_item, 2), (duplicate_fn, 4),
    (add_item, 8), (int_literal, 6), (str_literal, 4), (toggle_inline, 4),
    (doc_comment, 4), (reorder_derive, 2), (narrow_visibility, 2), (rename_local, 3),
]

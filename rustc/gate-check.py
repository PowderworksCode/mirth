#!/usr/bin/env python3
"""Feature gates: nothing unstable may be usable from stable code, whatever the spelling.

Two enumerations, each compiled without any `#![feature]` and with `RUSTC_BOOTSTRAP` unset
(as a stable user would):

  attributes  every attribute in the "Unstable attributes" part of
              compiler/rustc_feature/src/builtin_attrs.rs, placed on each kind of item and
              position (a function with and without a body, a trait method declaration, a
              foreign function, a parameter, a struct, a field, an impl, a module, a closure, a
              statement, the crate)
  library     every top-level `pub` item of core, alloc and std marked `#[unstable(feature)]`,
              reached by `use` (directly, renamed, through a glob), implemented (traits) and
              taken as a value (functions)

A program must report the gate (E0658, or "is experimental" / "unstable"). One that compiles is a
finding; one that fails without mentioning the gate is noted (an earlier error may have hidden
it). Library items whose path does not resolve even with the feature are skipped.

    rustc/gate-check.py --rustc <rustc> --rust <rust checkout> --work <dir> [--jobs 8]
        [--only attributes|library]
"""

import argparse
import json
import os
import re
import subprocess
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--rust", required=True)
p.add_argument("--work", required=True)
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--only")
args = p.parse_args()
WORK = Path(args.work).resolve()
WORK.mkdir(parents=True, exist_ok=True)
RUST = Path(args.rust)
ENV = {k: v for k, v in os.environ.items() if k != "RUSTC_BOOTSTRAP"}
GATE = re.compile(r"E0658|is experimental|is unstable|unstable feature|use of unstable|"
                  r"internal implementation detail|unstable library feature|requires a nightly|"
                  r"used internally by the standard library|may not be used|are considered unstable|"
                  r"is an internal|cannot be used on stable")

# Arguments for attributes that need them; the rest are written bare.
ATTR_ARGS = {
    "optimize": "(speed)", "patchable_function_entry": "(prefix_nops = 1, entry_nops = 1)",
    "instrument_fn": ' = "on"', "cfi_encoding": ' = "u1x"', "register_tool": "(mytool)",
    "register_attribute_tool": "(mytool)", "register_lint_tool": "(mytool)", "linkage": ' = "weak"',
    "lang": ' = "mirth_nonexistent"', "rustc_on_unimplemented": '(message = "x")',
    "rustc_diagnostic_item": ' = "mirth_x"', "test_runner": "(crate::r)", "pattern_complexity_limit": " = 10",
    "rustc_legacy_const_generics": "(0)", "rustc_layout_scalar_valid_range_start": "(1)",
    "rustc_abi": "(debug)", "rustc_macro_transparency": ' = "semitransparent"', "unstable": '(feature = "x", issue = "none")',
    "stable": '(feature = "x", since = "1.0.0")', "rustc_const_unstable": '(feature = "x", issue = "none")',
    "rustc_const_stable": '(feature = "x", since = "1.0.0")', "feature": "(mirth_nonexistent)",
    "rustc_objc_class": ' = "X"', "rustc_objc_selector": ' = "x"', "rustc_confusables": '("x")',
    "rustc_must_implement_one_of": "(a, b)", "rustc_deprecated_safe_2024": "",
    "allow_internal_unstable": "(core_intrinsics)", "rustc_allow_const_fn_unstable": "(x)",
    "rustc_default_body_unstable": '(feature = "x", issue = "none")', "unstable_removed": "",
    "rustc_simd_monomorphize_lane_limit": ' = "8"', "rustc_scalable_vector": "(4)",
}
# Each position: (name, program with {A} where the attribute goes).
POSITIONS = [
    ("fn", "{A}\npub fn f() {{}}\nfn main() {{}}"),
    ("fn-no-body", "pub trait T {{ {A} fn m(&self); }}\nfn main() {{}}"),
    ("foreign-fn", 'unsafe extern "C" {{ {A} fn ext(); }}\nfn main() {{}}'),
    ("param", "pub fn f({A} x: u32) -> u32 {{ x }}\nfn main() {{}}"),
    ("param-no-body", "pub trait T {{ fn m(&self, {A} x: u32); }}\nfn main() {{}}"),
    ("fn-ptr-param", "pub type F = fn({A} u32);\nfn main() {{}}"),
    ("struct", "{A}\npub struct S;\nfn main() {{}}"),
    ("field", "pub struct S {{ {A} pub x: u32 }}\nfn main() {{}}"),
    ("impl", "pub struct S;\n{A}\nimpl S {{}}\nfn main() {{}}"),
    ("trait", "{A}\npub trait T {{}}\nfn main() {{}}"),
    ("mod", "{A}\npub mod m {{}}\nfn main() {{}}"),
    ("closure", "fn main() {{ let _c = {A} || (); }}"),
    ("statement", "fn main() {{ {A} let _x = 1; }}"),
    ("crate", "#![{AI}]\nfn main() {{}}"),
]


def compile_text(text, feature=None, crate_type="bin"):
    with tempfile.TemporaryDirectory(dir=WORK) as d:
        f = Path(d) / "t.rs"
        f.write_text((f"#![feature({feature})]\n" if feature else "") + text)
        env = dict(ENV, RUSTC_BOOTSTRAP="1") if feature else ENV
        try:
            r = subprocess.run([args.rustc, str(f), "--edition", "2021", "--emit=metadata", "--crate-type",
                                crate_type, "-o", str(Path(d) / "x")], capture_output=True, text=True,
                               timeout=60, cwd=d, env=env)
        except subprocess.TimeoutExpired:
            return "timeout", ""
    if "internal compiler error" in r.stderr or "panicked" in r.stderr:
        return "ice", r.stderr
    return ("ok" if r.returncode == 0 else "error"), r.stderr


def classify(status, stderr, item=None):
    # `#[feature]` outside the crate root does nothing; rustc warns that it belongs at the root.
    if status == "ok" and item == "feature" and "crate-level attribute" in stderr:
        return "gated"
    if status == "ok":
        return "accepted"
    if status == "ice":
        return "ice"
    return "gated" if GATE.search(stderr) else "no-gate-message"


def attributes():
    text = (RUST / "compiler/rustc_feature/src/builtin_attrs.rs").read_text()
    names = sorted(set(re.findall(r"sym::([a-z_0-9]+)", text[text.index("Unstable attributes:"):])))
    jobs = []
    for name in names:
        arg = ATTR_ARGS.get(name, "")
        for pos, template in POSITIONS:
            jobs.append((name, pos, template.replace("{AI}", f"{name}{arg}").replace("{A}", f"#[{name}{arg}]")
                         .replace("{{", "{").replace("}}", "}")))

    def run(job):
        name, pos, prog = job
        status, err = compile_text(prog)
        return {"kind": "attribute", "item": name, "position": pos, "result": classify(status, err, name),
                "first": next((l for l in err.splitlines() if l.startswith("error")), "")[:200]}
    with ThreadPoolExecutor(args.jobs) as ex:
        return list(ex.map(run, jobs))


def library_items():
    items = []
    for krate in ("core", "alloc", "std"):
        root = RUST / "library" / krate / "src"
        for f in root.rglob("*.rs"):
            rel = f.relative_to(root).with_suffix("")
            parts = [x for x in rel.parts if x not in ("lib", "mod")]
            module = "::".join([krate] + parts)
            lines = f.read_text(errors="replace").splitlines()
            for i, line in enumerate(lines):
                m = re.match(r'#\[unstable\(feature = "([a-z_0-9]+)"', line)
                if not m:
                    continue
                for nxt in lines[i + 1:i + 6]:
                    if nxt.startswith("#"):
                        continue
                    d = re.match(r"pub (?:const |unsafe |auto |extern \"C\" )*(struct|enum|trait|union|type|fn|const|static|macro) ([A-Za-z_][A-Za-z_0-9]*)(<)?", nxt)
                    if d:
                        items.append({"crate": krate, "path": f"{module}::{d.group(2)}", "kind": d.group(1),
                                      "feature": m.group(1), "generic": bool(d.group(3)),
                                      "file": str(f.relative_to(RUST))})
                    break
    return items


def library():
    items = library_items()

    def run(item):
        path, feature, kind = item["path"], item["feature"], item["kind"]
        # The path must resolve with the feature on, or the item is not where its file says.
        status, _ = compile_text(f"#[allow(unused_imports)] use {path};\nfn main() {{}}", feature)
        if status != "ok":
            return [{"kind": "library", "item": path, "position": "path", "result": "skipped: path"}]
        parent, name = path.rsplit("::", 1)
        progs = [("use", f"#[allow(unused_imports)] use {path};\nfn main() {{}}"),
                 ("use-as", f"#[allow(unused_imports)] use {path} as Renamed;\nfn main() {{}}"),
                 ("glob", f"#[allow(unused_imports)] use {parent}::*;\n#[allow(unused_imports)] use self::{name} as _;\nfn main() {{}}")]
        if kind == "trait" and not item["generic"]:
            progs.append(("impl", f"struct L;\nimpl {path} for L {{}}\nfn main() {{}}"))
        if kind == "fn" and not item["generic"]:
            progs.append(("value", f"fn main() {{ let _f = {path}; }}"))
        if kind in ("struct", "enum", "union", "type") and not item["generic"]:
            progs.append(("type", f"pub fn g(_: Option<&{path}>) {{}}\nfn main() {{}}"))
        out = []
        for pos, prog in progs:
            status, err = compile_text(prog)
            out.append({"kind": "library", "item": path, "feature": feature, "position": pos,
                        "result": classify(status, err),
                        "first": next((l for l in err.splitlines() if l.startswith("error")), "")[:200]})
        return out
    with ThreadPoolExecutor(args.jobs) as ex:
        return [r for rs in ex.map(run, items) for r in rs]


def main():
    results = []
    if args.only in (None, "attributes"):
        results += attributes()
    if args.only in (None, "library"):
        results += library()
    (WORK / "results.json").write_text(json.dumps(results, indent=0))
    from collections import Counter
    print(Counter((r["kind"], r["result"]) for r in results))
    for r in results:
        if r["result"] in ("accepted", "ice"):
            print(f"{r['result'].upper():9} {r['kind']:9} {r['item']:45} {r['position']}")


main()

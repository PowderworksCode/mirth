#!/usr/bin/env python3
"""ABI differential: rustc's `extern "C"` must lower a signature the way clang lowers the same C
signature, on every target both support.

Generates random C signatures (bool, integers of each width, float, double, pointers, __int128
on 64-bit targets, and repr(C) structs, unions and arrays of them, nested, packed or
over-aligned), writes each as a C function (compiled by clang for the target's LLVM triple) and
as a Rust `#[no_mangle] extern "C" fn` (compiled by rustc for the target against minicore, so no
sysroot is needed), and compares the two LLVM IR signatures, parameter by parameter after
first-class aggregates are flattened (LLVM assigns their elements to registers one by one):

  finding  a parameter or the return value differs in its register class (integer, floating
           point, vector, memory), in an extension attribute (zeroext, signext), in inreg,
           byval or sret, in the alignment of a byval or sret pointer, in the number of
           parameters, or in the calling convention
  note     the same register classes with different IR types (`{double, double}` against
           `float, double`), or a `noundef` difference

    rustc/abi-diff.py --rustc <rustc> --rust <rust checkout> --work <dir> [--targets t1,t2 | --all]
        [--count 200] [--seed 1] [--jobs 8] [--clang clang]

Known bugs are labelled, not reported: rust-lang/rust#163911 (x86_64 bool returns) and findings
19 and 20 in docs/hunt.md (RISC-V and LoongArch); so are two differences this host cannot decide
(i686 MSVC small-struct returns, a PowerPC64 `inreg` float).

Writes <work>/<target>/{a.c,a.rs,c.ll,r.ll} and <work>/results.json (per target: functions
compared, findings, notes), and prints the findings grouped by kind.
"""

import argparse
import ast
import json
import random
import re
import subprocess
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import os

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--rust", required=True)
p.add_argument("--work", required=True)
p.add_argument("--targets")
p.add_argument("--all", action="store_true", help="every target rustc knows; the default is MAIN, the tier 1 and "
               "2 targets whose differences have been triaged (the others still show representation differences)")
p.add_argument("--count", type=int, default=200)
p.add_argument("--seed", type=int, default=1)
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--clang", default="clang")
args = p.parse_args()
WORK = Path(args.work).resolve()
WORK.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ, RUSTC_BOOTSTRAP="1")

SCALARS = [  # (C, Rust)
    ("_Bool", "bool"), ("signed char", "i8"), ("unsigned char", "u8"), ("short", "i16"),
    ("unsigned short", "u16"), ("int", "i32"), ("unsigned int", "u32"), ("long long", "i64"),
    ("unsigned long long", "u64"), ("float", "f32"), ("double", "f64"), ("void*", "*mut u8"),
]
WIDE = [("__int128", "i128"), ("unsigned __int128", "u128")]


class Gen:
    def __init__(self, rng, wide):
        self.rng, self.wide, self.structs, self.names = rng, wide, [], 0

    def scalar(self):
        pool = SCALARS + (WIDE if self.wide else [])
        return self.rng.choice(pool)

    def field_type(self, depth):
        r = self.rng.random()
        if depth < 2 and r < 0.15:
            return self.aggregate(depth + 1)
        if r < 0.25:
            c, rs = self.scalar()
            n = self.rng.choice([1, 2, 3, 4, 8])
            return ("array", c, rs, n)
        return self.scalar()

    def aggregate(self, depth=0):
        name = f"S{self.names}"
        self.names += 1
        union = self.rng.random() < 0.12
        packed = not union and self.rng.random() < 0.08
        # Rust rejects packed with align, and a packed type containing an over-aligned one:
        # packed structs get scalars and arrays only.
        align = None if packed else self.rng.choice([None] * 9 + [16, 32])
        fields = [self.field_type(2 if packed else depth) for _ in range(self.rng.randint(1, 5))]
        self.structs.append((name, union, packed, align, fields))
        return (name, name)

    def ty(self):
        return self.aggregate() if self.rng.random() < 0.4 else self.scalar()


def c_field(i, f):
    if f[0] == "array":
        return f"{f[1]} f{i}[{f[3]}];"
    c = f[0]
    return f"{c} f{i};"


def r_field(i, f):
    if f[0] == "array":
        return f"pub f{i}: [{f[2]}; {f[3]}],"
    return f"pub f{i}: {f[1]},"


def c_ty(t):
    return t[0]


def program(target_wide, seed):
    rng = random.Random(seed)
    g = Gen(rng, target_wide)
    fns = []
    for k in range(args.count):
        params = [g.ty() for _ in range(rng.randint(0, 8))]
        ret = None if rng.random() < 0.15 else g.ty()
        fns.append((f"f{k}", params, ret))
    c = ["#include <stdbool.h>"]
    rs = ["#![feature(no_core)]", "#![no_core]", '#![crate_type = "lib"]',
          "#![allow(improper_ctypes_definitions, unused, non_snake_case)]",
          "extern crate minicore;", "use minicore::*;"]
    for name, union, packed, align, fields in g.structs:
        kw = "union" if union else "struct"
        attrs = (" __attribute__((packed))" if packed else "") + (f" __attribute__((aligned({align})))" if align else "")
        c.append(f"typedef {kw} {name} {{ {' '.join(c_field(i, f) for i, f in enumerate(fields))} }}{attrs} {name};")
        repr_ = "C" + (", packed" if packed else "") + (f", align({align})" if align else "")
        rs.append(f"#[repr({repr_})] pub {kw} {name} {{ {' '.join(r_field(i, f) for i, f in enumerate(fields))} }}")
        # Union fields must be Copy; minicore has no derive.
        rs.append(f"impl Copy for {name} {{}}")
    for name, params, ret in fns:
        cp = ", ".join(f"{c_ty(t)} a{i}" for i, t in enumerate(params)) or "void"
        c.append(f"{c_ty(ret) if ret else 'void'} {name}({cp}) {{ for (;;); }}")
        rp = ", ".join(f"a{i}: {t[1]}" for i, t in enumerate(params))
        rr = f" -> {ret[1]}" if ret else ""
        rs.append(f"#[no_mangle] pub extern \"C\" fn {name}({rp}){rr} {{ loop {{}} }}")
    return "\n".join(c) + "\n", "\n".join(rs) + "\n", fns


DEFINE = re.compile(r"^define\s+(.*?)@(\w+)\((.*)\)(.*)\{\s*$")
DROP = re.compile(r"\b(noundef|nonnull|noalias|nocapture|readonly|readnone|writeonly|writable|dead_on_unwind|"
                  r"captures\([^)]*\)|dereferenceable(_or_null)?\(\d+\)|initializes\([^)]*\)|range\([^)]*\)|"
                  r"nofpclass\([^)]*\)|immarg|returned|local_unnamed_addr|unnamed_addr|dso_local|"
                  r"dso_preemptable|hidden|protected|internal|private|nounwind|noinline|optnone|"
                  r"!\w+ !\d+|#\d+)\b")


def split_top(s):
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch in "({[<":
            depth += 1
        elif ch in ")}]>":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def param_type(p):
    """The IR type of a parameter declaration, its attributes, and whether it has noundef."""
    p = re.sub(r"\s+%[\w.]+$", "", p.strip())
    noundef = "noundef" in p
    m = re.match(r"^(\{[^}]*\}|\[[^\]]*\]|<[^>]*>|[\w.%*]+)(.*)$", p)
    ty, attrs = (m.group(1), m.group(2)) if m else (p, "")
    attrs = DROP.sub("", attrs)
    kept = []
    for a in re.findall(r"(zeroext|signext|inreg|byval\([^)]*\)|sret\([^)]*\)|byref\([^)]*\)|align \d+|inalloca\([^)]*\))", attrs):
        kept.append(re.sub(r"\(.*\)$", "", a) if a.startswith(("byval", "sret", "byref", "inalloca")) else a)
    # An `align` on a plain pointer is a hint, not ABI; on byval and sret it is ABI.
    if not any(k.startswith(("byval", "sret", "byref")) for k in kept):
        kept = [k for k in kept if not k.startswith("align")]
    return ty, tuple(sorted(kept)), noundef


def flatten(ty, named):
    """A type's register-assignable parts; `named` maps the module's %struct names to bodies."""
    ty = ty.strip()
    if ty in named:
        ty = named[ty]
    if ty.startswith("<{") and ty.endswith("}>"):  # packed struct
        ty = ty[1:-1]
    if ty.startswith("{") and ty.endswith("}"):
        return [x for part in split_top(ty[1:-1]) for x in flatten(part.strip(), named)]
    m = re.fullmatch(r"\[1 x (.*)\]", ty)
    if m:  # a one-element array goes where its element goes
        return flatten(m.group(1), named)
    return [ty]


def units(types, arch=None, ret=False, width=64):
    """Register classes, one per register-sized unit: an integer wider than a register takes
    several. For a return value, an array is its elements (returned in consecutive registers)."""
    out = []
    for t in types:
        m = re.fullmatch(r"\[(\d+) x (i\d+|float|double)\]", t)
        mi = re.fullmatch(r"i(\d+)", t)
        if mi and int(mi.group(1)) > width:
            out += ["int"] * (int(mi.group(1)) // width)
        # A float array is a homogeneous aggregate: in a return, or an ARM VFP argument, it
        # takes consecutive floating-point registers like a struct of its elements.
        elif m and (ret or (arch == "arm" and m.group(2) in ("float", "double"))):
            out += units([m.group(2)] * int(m.group(1)), arch, ret, width)
        elif t == "agg":
            out.append("agg")
        else:
            k = klass(t)
            # x86's SSE registers hold floats, doubles and vectors alike.
            if arch in ("x86_64", "x86") and k in ("fp", "vec"):
                k = "sse"
            out.append(k)
    return out


def klass(ty):
    if ty in ("float", "double", "half", "bfloat", "fp128", "x86_fp80", "ppc_fp128") or ty.startswith("<"):
        return "fp" if not ty.startswith("<") else "vec"
    if ty.startswith("["):
        m = re.match(r"\[(\d+) x (.*)\]", ty)
        return f"[{m.group(1)} x {klass(m.group(2))}]" if m else ty
    if ty == "void":
        return "void"
    return "int"


def signature(line, named):
    m = DEFINE.match(line)
    if not m:
        return None
    head, name, params, tail = m.groups()
    cc = re.findall(r"\b(\w+cc|cc \d+)\b", head)
    head = DROP.sub("", head).strip()
    tm = re.search(r"(\{[^{}]*\}|<\{[^{}]*\}>|\[[^\]]*\]|<[^>]*>|[\w.%*]+)\s*$", head)
    ret_ty = tm.group(1) if tm else "void"
    ret_attrs = tuple(sorted(a for a in head[:tm.start() if tm else 0].split() if a in ("zeroext", "signext", "inreg")))
    ps = []
    for p in split_top(params):
        ty, attrs, noundef = param_type(p)
        for t in flatten(ty, named):
            ps.append((t, attrs, noundef))
    rets = flatten(ret_ty, named)
    return {"name": name, "cc": tuple(cc), "ret": rets, "ret_attrs": ret_attrs, "params": ps,
            "ret_noundef": "noundef" in m.group(1)}


def signatures(ll):
    named = {m.group(1): m.group(2).strip() for m in re.finditer(r"^(%[\w.]+) = type (.*)$", ll, re.M)}
    return {s["name"]: s for s in (signature(l, named) for l in ll.splitlines() if l.startswith("define")) if s}


def compare(c, r, arch, slot):
    """`arch`: the target's arch; `slot`: its stack slot size in bytes (the pointer width)."""
    findings, notes = [], []
    if c["cc"] != r["cc"]:
        findings.append(f"calling convention: clang {c['cc']} rustc {r['cc']}")
    if c["ret_attrs"] != r["ret_attrs"]:
        what = f"return attributes: clang {c['ret_attrs']} rustc {r['ret_attrs']}"
        # x86's psABI leaves the bits above a small integer return undefined (rustc stopped
        # extending them in #142389); only bool's bits 1-7 must be zero (#163911).
        if arch in ("x86_64", "x86") and c["ret"] != ["i1"]:
            notes.append(what + " (x86 psABI: upper bits undefined)")
        else:
            findings.append(what)
    if units(c["ret"], arch, True, slot * 8) != units(r["ret"], arch, True, slot * 8):
        findings.append(f"return: clang {c['ret']} rustc {r['ret']}")
    elif c["ret"] != r["ret"]:
        notes.append(f"return types: clang {c['ret']} rustc {r['ret']}")
    cp, rp = c["params"], r["params"]
    # ARM and 64-bit PowerPC split a byval aggregate between registers and the stack as they do
    # an array argument of the same size: both are "an aggregate".
    if arch in ("arm", "powerpc64"):
        agg = lambda t, a: ("agg", (), False) if ("byval" in a or re.match(r"\[\d+ x i\d+\]", t)) else (t, a, None)
        cp = [agg(t, a) if agg(t, a)[0] == "agg" else (t, a, n) for t, a, n in cp]
        rp = [agg(t, a) if agg(t, a)[0] == "agg" else (t, a, n) for t, a, n in rp]
    if units([t for t, _, _ in cp], arch, False, slot * 8) != units([t for t, _, _ in rp], arch, False, slot * 8):
        what = f"parameters: clang {[t for t, _, _ in cp]} rustc {[t for t, _, _ in rp]}"
        # i386 passes every argument on the stack: a struct expanded into its scalars and a
        # byval copy of it are the same bytes.
        if arch == "x86" and any("byval" in a for _, a, _ in cp + rp):
            notes.append(what + " (x86: same stack bytes)")
        else:
            findings.append(what)
    elif [t for t, _, _ in cp] != [t for t, _, _ in rp]:
        notes.append(f"parameter types: clang {[t for t, _, _ in cp]} rustc {[t for t, _, _ in rp]}")
    elif True:
        for i, ((ct, ca, cn), (rt, ra, rn)) in enumerate(zip(cp, rp)):
            if ca != ra:
                what = f"parameter {i} attributes: clang {ca} rustc {ra} ({ct})"
                aligns = [int(a.split()[1]) for a in ca + ra if a.startswith("align ")]
                rest_c = [a for a in ca if not a.startswith("align ")]
                rest_r = [a for a in ra if not a.startswith("align ")]
                ext_only_rust = not rest_c and set(rest_r) <= {"zeroext", "signext"}
                if rest_c == rest_r and aligns and max(aligns) <= slot:
                    # A byval copy goes in stack slots at least `slot` bytes aligned either way.
                    notes.append(what + " (both within a stack slot)")
                elif arch in ("wasm32", "wasm64") and {*rest_c, *rest_r} <= {"byval"}:
                    # WebAssembly lowers byval to a pointer to a copy the caller makes.
                    notes.append(what + " (wasm: byval is a pointer to a copy)")
                elif arch == "x86" and {*rest_c, *rest_r} <= {"byval"}:
                    # i386 passes everything on the stack: a byval copy and a struct expanded
                    # into its scalars occupy the same bytes.
                    notes.append(what + " (x86: same stack bytes)")
                elif arch == "x86_64" and ext_only_rust:
                    # Win64: rustc extends small integers, clang does not; LLVM does not rely on
                    # it in the callee (checked: both re-extend), so nothing observable.
                    notes.append(what + " (win64: extension not relied on)")
                else:
                    findings.append(what)
            if ct != rt:
                notes.append(f"parameter {i}: clang {ct} rustc {rt}")
            if cn != rn:
                notes.append(f"parameter {i} noundef: clang {cn} rustc {rn}")
    return findings, notes


MAIN = ("x86_64-unknown-linux-gnu x86_64-unknown-linux-musl x86_64-pc-windows-msvc x86_64-pc-windows-gnu "
        "x86_64-apple-darwin i686-unknown-linux-gnu i686-pc-windows-msvc aarch64-unknown-linux-gnu "
        "aarch64-apple-darwin aarch64-pc-windows-msvc aarch64-unknown-linux-musl armv7-unknown-linux-gnueabihf "
        "arm-unknown-linux-gnueabi thumbv7em-none-eabihf riscv64gc-unknown-linux-gnu riscv32imac-unknown-none-elf "
        "loongarch64-unknown-linux-gnu powerpc64le-unknown-linux-gnu s390x-unknown-linux-gnu wasm32-unknown-unknown "
        "wasm32-wasip1").split()


def known(arch, finding):
    """A finding that is a bug already recorded: the label, or None."""
    if arch == "x86_64" and finding.startswith("return attributes: clang ('zeroext',)"):
        return "rust-lang/rust#163911"
    if arch in ("riscv64", "riscv32", "loongarch64"):
        if re.match(r"parameter \d+ attributes: clang \('(signext|zeroext)',\) rustc \(\)", finding):
            return "finding 19 (docs/hunt.md)"
        if finding.startswith(("parameters:", "return:")):
            c, _, r = finding.partition(" rustc ")
            fp = lambda text: len(re.findall(r"'(float|double)'", text))
            if fp(r) > fp(c):
                return "finding 20 (docs/hunt.md)"
    return None


def unresolved(arch, target, finding):
    """A difference whose correct side needs a reference this host lacks: the label, or None."""
    lists = re.fullmatch(r"parameters: clang (\[.*?\]) rustc (\[.*\])", finding)
    sret_only = bool(lists) and (lambda c, r: c[:1] == ["ptr"] and c[1:] == r)(*map(ast.literal_eval, lists.groups()))
    if target.endswith("windows-msvc") and arch == "x86" and (
            (finding.startswith("return:") and "'void'" in finding) or sret_only):
        # clang returns an 8-byte struct with an array field of 3 bytes indirectly (its
        # register-size rule recurses into fields); rustc and MSVC's documentation return any
        # 8-byte struct in edx:eax. Needs MSVC to decide.
        return "i686 msvc small-struct return (needs MSVC)"
    if arch == "powerpc64" and re.match(r"parameter \d+ attributes: clang \('inreg',\) rustc \(\) \((float|double)\)", finding):
        # clang marks a float from a single-member aggregate inreg; whether the PowerPC backend
        # then places it differently needs a run on the target.
        return "ppc64 inreg float (needs a run)"
    return None


def run(argv, cwd):
    try:
        r = subprocess.run(argv, capture_output=True, text=True, timeout=600, cwd=cwd, env=ENV)
        return r.returncode, r.stderr
    except subprocess.TimeoutExpired:
        return -1, "timeout"


def spec(target):
    out = subprocess.run([args.rustc, "--print", "target-spec-json", "-Zunstable-options", "--target", target],
                         capture_output=True, text=True, env=ENV)
    return json.loads(out.stdout) if out.returncode == 0 else None


def one_target(target):
    s = spec(target)
    if not s:
        return target, {"skip": "no target spec"}
    width = int(s.get("target-pointer-width", 64))
    if width < 32 or s.get("c-int-width", "32") != "32":
        return target, {"skip": "16-bit int"}
    d = WORK / target
    d.mkdir(parents=True, exist_ok=True)
    csrc, rsrc, fns = program(width == 64 and s.get("arch") not in ("sparc64",), args.seed)
    (d / "a.c").write_text(csrc)
    (d / "a.rs").write_text(rsrc)
    base = [args.rustc, "--target", target, "-Zunstable-options", "--edition", "2021", "-Cpanic=abort",
            "--out-dir", str(d)]
    code, err = run(base + ["--crate-type", "rlib", "--crate-name", "minicore", "-Awarnings",
                            str(Path(args.rust) / "tests/auxiliary/minicore.rs")], d)
    if code:
        return target, {"skip": "minicore does not build", "err": err[-500:]}
    code, err = run(base + ["--emit=llvm-ir", "-Copt-level=0", "--extern", f"minicore={d}/libminicore.rlib",
                            "-o", str(d / "r.ll"), str(d / "a.rs")], d)
    if code:
        return target, {"skip": "rust side does not build", "err": err[-1500:]}
    cflags = []
    # The target's CPU and features decide parts of the ABI in clang too (soft-float, SSE).
    if s.get("cpu") and s["cpu"] != "generic":
        cflags += ["-Xclang", "-target-cpu", "-Xclang", s["cpu"]]
    for feature in filter(None, s.get("features", "").split(",")):
        cflags += ["-Xclang", "-target-feature", "-Xclang", feature]
    if s.get("llvm-abiname"):
        cflags.append(f"-mabi={s['llvm-abiname']}")
    if s.get("llvm-floatabi") == "hard":
        cflags.append("-mfloat-abi=hard")
    code, err = run([args.clang, f"--target={s['llvm-target']}", "-ffreestanding", "-S", "-emit-llvm", "-O0",
                     "-Wno-everything", *cflags, "-o", str(d / "c.ll"), str(d / "a.c")], d)
    if code:
        return target, {"skip": "clang does not build", "err": err[-800:]}
    cs, rs = signatures((d / "c.ll").read_text()), signatures((d / "r.ll").read_text())
    findings, notes, known_hits = {}, {}, {}
    for name, _, _ in fns:
        if name not in cs or name not in rs:
            continue
        f, n = compare(cs[name], rs[name], s.get("arch"), width // 8)
        labelled = [(x, known(s.get("arch"), x) or unresolved(s.get("arch"), target, x)) for x in f]
        f = [x for x, k in labelled if not k]
        for x, k in labelled:
            if k:
                known_hits.setdefault(k, 0)
                known_hits[k] += 1
        if f:
            findings[name] = f
        if n:
            notes[name] = n
    return target, {"compared": len(fns), "findings": findings, "notes": notes, "known": known_hits}


def main():
    if args.targets:
        targets = args.targets.split(",")
    elif not args.all:
        targets = MAIN
    else:
        targets = subprocess.run([args.rustc, "--print", "target-list"], capture_output=True, text=True,
                                 env=ENV).stdout.split()
    with ThreadPoolExecutor(args.jobs) as ex:
        results = dict(ex.map(one_target, targets))
    (WORK / "results.json").write_text(json.dumps(results, indent=1))
    kinds = defaultdict(lambda: defaultdict(int))
    skipped = defaultdict(list)
    for t, r in results.items():
        if "skip" in r:
            skipped[r["skip"]].append(t)
            continue
        for fs in r["findings"].values():
            for f in fs:
                kinds[re.sub(r"\(.*|:.*", "", f).strip()][t] += 1
    compared = [t for t, r in results.items() if "skip" not in r]
    knowns = defaultdict(int)
    for r in results.values():
        for k, n in r.get("known", {}).items():
            knowns[k] += n
    if knowns:
        print("known: " + ", ".join(f"{k} {n}" for k, n in sorted(knowns.items())))
    print(f"{len(compared)} targets compared; skipped: " + ", ".join(f"{k} {len(v)}" for k, v in skipped.items()))
    for kind, per in sorted(kinds.items(), key=lambda kv: -sum(kv[1].values())):
        print(f"{kind}: {sum(per.values())} in {len(per)} targets: "
              + ", ".join(f"{t} {n}" for t, n in sorted(per.items(), key=lambda kv: -kv[1])[:8]))


main()

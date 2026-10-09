#!/usr/bin/env python3
"""Compiler runs that rustc's test suites hardly make, for coverage (rustc/callgraph.py --gaps
lists what is left): run them with the coverage-instrumented compiler and MIRTH_OUT set, and fold
the logs with rustc/coverage-compact.py.

    MIRTH_OUT=<logs> rustc/coverage-generators.py --rustc <rustc> --rust <rust checkout>
        --list picked.json --work <dir> [--jobs 8] [--only prints,targets,dumps]

- prints: every `--print` request, on the host and on every target;
- targets: `tests/auxiliary/minicore.rs` and a file of functions with every kind of argument
  and return value, compiled to an object for every target (each target's ABI, layout and
  codegen code);
- links: for every target, a `no_main` binary, a cdylib, a staticlib and a dylib on minicore,
  linked with `-Clinker=true` (the linker command each target's linker flavor builds, without
  the linker), with linker options;
- dumps: each test of the list (rustc/ui-coverage.py pick) with each debugging and printing
  option (`-Zunpretty=`, `-Zdump-mir`, `-Zprint-type-sizes`, statistics, profiling, ...).
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
p.add_argument("--list", required=True)
p.add_argument("--work", required=True)
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--only", default="prints,targets,dumps,links")
args = p.parse_args()
WORK = Path(args.work)
WORK.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ, RUSTC_BOOTSTRAP="1")

PRINTS = ["all-target-specs-json", "backend-has-mnemonic", "backend-has-zstd", "calling-conventions",
          "cfg", "check-cfg", "code-models", "crate-name", "crate-root-lint-levels", "deployment-target",
          "file-names", "host-tuple", "link-args", "native-static-libs", "relocation-models",
          "split-debuginfo", "stack-protector-strategies", "supported-crate-types", "sysroot",
          "target-cpus", "target-features", "target-libdir", "target-list", "target-spec-json",
          "target-spec-json-schema", "tls-models", "wasm-proc-macro-tuple"]
PER_TARGET = ["cfg", "target-spec-json", "target-cpus", "target-features", "calling-conventions",
              "code-models", "relocation-models", "tls-models", "stack-protector-strategies",
              "split-debuginfo", "supported-crate-types", "deployment-target", "check-cfg"]

# Every kind of argument and return value, for each target's calling convention.
ABI = r"""
#![feature(no_core, lang_items, rustc_attrs, c_variadic, f16, f128)]
#![no_core]
#![crate_type = "lib"]
#![allow(improper_ctypes_definitions, unused)]
extern crate minicore;
use minicore::*;

#[repr(C)] pub struct Small { a: u8, b: u16 }
#[repr(C)] pub struct Pair { a: u64, b: u64 }
#[repr(C)] pub struct Big { a: [u64; 8] }
#[repr(C)] pub struct Floats { a: f32, b: f64 }
#[repr(C)] pub struct Mixed { a: f32, b: u32 }
#[repr(C)] pub union U { a: u32, b: f32 }
#[repr(C)] pub struct Hfa { a: f32, b: f32, c: f32, d: f32 }
#[repr(C)] pub struct Empty {}
#[repr(transparent)] pub struct T(u64);
#[repr(C, packed)] pub struct Packed { a: u8, b: u32 }
#[repr(C, align(16))] pub struct Aligned { a: u8 }

#[no_mangle] pub extern "C" fn c_small(x: Small) -> Small { x }
#[no_mangle] pub extern "C" fn c_pair(x: Pair) -> Pair { x }
#[no_mangle] pub extern "C" fn c_big(x: Big) -> Big { x }
#[no_mangle] pub extern "C" fn c_floats(x: Floats, y: f32, z: f64) -> Floats { x }
#[no_mangle] pub extern "C" fn c_mixed(x: Mixed) -> Mixed { x }
#[no_mangle] pub extern "C" fn c_union(x: U) -> U { x }
#[no_mangle] pub extern "C" fn c_hfa(x: Hfa) -> Hfa { x }
#[no_mangle] pub extern "C" fn c_empty(x: Empty) -> Empty { x }
#[no_mangle] pub extern "C" fn c_transparent(x: T) -> T { x }
#[no_mangle] pub extern "C" fn c_packed(x: Packed) -> Packed { x }
#[no_mangle] pub extern "C" fn c_aligned(x: Aligned) -> Aligned { x }
#[no_mangle] pub extern "C" fn c_ints(a: i8, b: u16, c: i32, d: u64, e: i128, f: u128, g: bool, h: char) -> i128 { e }
#[no_mangle] pub extern "C" fn c_ptrs(a: *const u8, b: &u32, c: &mut [u8; 3], f: extern "C" fn()) -> *const u8 { a }
#[no_mangle] pub extern "C" fn c_many(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64, h: u64, i: u64, j: Pair, k: f64, l: f64, m: f64, n: f64, o: f64, p: f64, q: f64, r: f64, s: f64) -> u64 { a }
#[no_mangle] pub unsafe extern "C" fn c_variadic(a: u32, mut args: ...) -> u32 { a }
pub fn rust_all(a: Small, b: Pair, c: Big, d: Floats, e: (u8, u64), f: [u32; 5], g: &[u8], h: &str, i: u128) -> Big { c }
pub fn rust_f16(a: f16, b: f128) -> f128 { b }
#[no_mangle] pub extern "C" fn c_f16(a: f16, b: f128) -> f128 { b }
#[no_mangle] pub extern "system" fn system(a: Pair) -> Pair { a }
#[no_mangle] pub extern "C-unwind" fn c_unwind(a: Pair) -> Pair { a }
pub static TABLE: [extern "C" fn(Pair) -> Pair; 2] = [c_pair, c_unwind_shim];
extern "C" fn c_unwind_shim(a: Pair) -> Pair { a }
extern "C" { fn imported(a: Big, b: Floats) -> Hfa; }
pub unsafe fn call_imported(a: Big, b: Floats) -> Hfa { imported(a, b) }
"""


def run(argv, cwd=None, timeout=300):
    try:
        r = subprocess.run(argv, capture_output=True, text=True, timeout=timeout, cwd=cwd, env=ENV)
        return r.returncode, r.stderr
    except subprocess.TimeoutExpired:
        return -1, "timeout"


def targets():
    out = subprocess.run([args.rustc, "--print", "target-list"], capture_output=True, text=True, env=ENV)
    return out.stdout.split()


def prints():
    jobs = [[args.rustc, "--print", kind, "-Zunstable-options", "-"] for kind in PRINTS]
    for target in targets():
        for kind in PER_TARGET:
            jobs.append([args.rustc, "--print", kind, "--target", target, "-Zunstable-options", "-"])
    with tempfile.TemporaryDirectory(dir=WORK) as d:
        empty = Path(d) / "lib.rs"
        empty.write_text("")
        jobs = [[a if a != "-" else str(empty) for a in j] for j in jobs]
        with ThreadPoolExecutor(args.jobs) as ex:
            done = list(ex.map(lambda j: run(j, cwd=d), jobs))
    print(f"prints: {len(jobs)} runs, {sum(c == 0 for c, _ in done)} succeeded", flush=True)


def one_target(target):
    with tempfile.TemporaryDirectory(dir=WORK) as d:
        minicore = Path(args.rust) / "tests/auxiliary/minicore.rs"
        (Path(d) / "abi.rs").write_text(ABI)
        results = []
        base = [args.rustc, "--target", target, "-Zunstable-options", "--edition", "2021",
                "-Cpanic=abort", "--out-dir", d]
        code, err = run(base + ["--crate-type", "rlib", "--crate-name", "minicore", "-Copt-level=1",
                                "--emit=link,obj", str(minicore)], cwd=d)
        results.append(code)
        if code == 0:
            for opt in ("0", "3"):
                code, err = run(base + ["--emit=obj,asm,llvm-ir", f"-Copt-level={opt}", "-Cdebuginfo=2",
                                        "--extern", f"minicore={d}/libminicore.rlib", "abi.rs"], cwd=d)
                results.append(code)
        return target, results


def cross():
    with ThreadPoolExecutor(args.jobs) as ex:
        done = list(ex.map(one_target, targets()))
    built = sum(all(c == 0 for c in r) and len(r) == 3 for _, r in done)
    print(f"targets: {len(done)} targets, {built} built minicore and the ABI file", flush=True)
    (WORK / "targets.json").write_text(json.dumps(dict(done), indent=0))


LINKED = r"""
#![feature(no_core, lang_items)]
#![no_core]
#![no_main]
extern crate minicore;
#[no_mangle] pub extern "C" fn exported(a: u32) -> u32 { a }
#[no_mangle] pub static DATA: u32 = 7;
#[link(name = "c")] extern "C" { fn puts(p: *const u8) -> i32; }
#[link(name = "m", kind = "static")] extern "C" {}
#[link(name = "framework_like", kind = "dylib", modifiers = "+verbatim")] extern "C" {}
"""
LINK_OPTIONS = [[], ["-Cprefer-dynamic", "-Crelocation-model=pic"], ["-Cstrip=symbols", "-Clink-dead-code"],
                ["-Clink-self-contained=yes"], ["-Cdebuginfo=2", "-Csplit-debuginfo=packed"],
                ["-Clink-arg=-Wl,--foo", "-Clink-args=-x -y", "-Zpre-link-args=-z"],
                ["-Cdefault-linker-libraries", "-Zlink-native-libraries=no"], ["-Ccontrol-flow-guard"],
                ["-Zstaticlib-allow-rdylib-deps"], ["-Copt-level=s", "-Clto=fat"], ["-Ccode-model=large"]]


def one_link(target):
    with tempfile.TemporaryDirectory(dir=WORK) as d:
        minicore = Path(args.rust) / "tests/auxiliary/minicore.rs"
        (Path(d) / "linked.rs").write_text(LINKED)
        base = [args.rustc, "--target", target, "-Zunstable-options", "--edition", "2021", "-Cpanic=abort",
                "--out-dir", d, "-Clinker=true"]
        code, _ = run(base + ["--crate-type", "rlib", "--crate-name", "minicore", "--emit=link",
                              str(minicore)], cwd=d)
        ok = 0
        if code == 0:
            for options in LINK_OPTIONS:
                for kind in ("bin", "cdylib", "staticlib", "dylib"):
                    c, _ = run(base + ["--crate-type", kind, "--extern", f"minicore={d}/libminicore.rlib",
                                       "-Csave-temps", *options, "linked.rs"], cwd=d)
                    ok += c == 0
        return ok


def links():
    with ThreadPoolExecutor(args.jobs) as ex:
        done = list(ex.map(one_link, targets()))
    print(f"links: {len(done)} targets, {sum(done)} links succeeded", flush=True)


DUMPS = [[f"-Zunpretty={m}"] for m in ("normal", "expanded", "expanded,identified", "expanded,hygiene",
                                       "ast-tree", "ast-tree,expanded", "hir", "hir,identified",
                                       "hir,typed", "hir-tree", "thir-tree", "thir-flat", "mir",
                                       "stable-mir", "mir-cfg")] + [
    ["-Zdump-mir=all", "-Zdump-mir-dataflow", "-Zdump-mir-graphviz", "-Zmir-include-spans=on"],
    ["-Zprint-type-sizes"], ["-Zprint-mono-items=yes", "--emit=link"], ["-Zmeta-stats"], ["-Zhir-stats"],
    ["-Zinput-stats"], ["-Zself-profile", "-Zself-profile-events=all"], ["-Ztime-passes"],
    ["-Zquery-dep-graph", "-Zdump-dep-graph", "-Cincremental=inc"], ["-Zincremental-info", "-Cincremental=inc"],
    ["-Zdump-mono-stats", "-Zdump-mono-stats-format=json", "--emit=link"], ["-Zprint-codegen-stats", "--emit=link"],
    ["-Zvalidate-mir", "-Zlint-mir", "-Zmir-opt-level=4"], ["-Zverbose-internals", "-Zidentify-regions"],
    ["-Ztrack-diagnostics", "-Zteach"], ["-Zthreads=4"], ["-Zpolonius=next"], ["-Zinline-mir", "-Zmir-opt-level=3"],
    ["-Zrandomize-layout"], ["-Zwrite-long-types-to-disk=no", "-Zverbose-internals"],
    ["-Zunleash-the-miri-inside-of-you"], ["-Zno-analysis"], ["-Zprofile-closures"], ["-Zui-testing"],
    ["-Cinstrument-coverage", "--emit=link"], ["-Zemit-stack-sizes", "--emit=link"],
    ["--error-format=json", "--json=diagnostic-rendered-ansi,artifacts,future-incompat,unused-externs"],
    ["--error-format=human-annotate-rs"], ["--error-format=short"], ["-Zterminal-urls=yes", "--color=always"],
    ["-Wunused", "-Wrust-2018-idioms", "-Wrust-2021-compatibility", "-Wrust-2024-compatibility", "-Wclippy::all"],
    ["-Fwarnings", "--cap-lints=warn"], ["-Zcodegen-source-order", "--emit=link"],
]


def headers(text):
    flags, edition, revision = [], None, None
    revs = re.search(r"^//@\s*revisions:\s*(.*)$", text, re.M)
    if revs:
        revision = revs.group(1).split()[0]
    for m in re.finditer(r"^//@(?:\[([\w,-]+)\])?\s*([a-z-]+)(?::\s*(.*))?$", text, re.M):
        only, key, value = m.group(1), m.group(2), (m.group(3) or "").strip()
        if only and (revision is None or revision not in only.split(",")):
            continue
        if key == "compile-flags":
            flags += value.split()
        elif key == "edition":
            edition = value.split()[0]
    if revision:
        flags += ["--cfg", revision]
    return flags, edition


def dump(test):
    path = Path(args.rust) / "tests/ui" / test
    text = path.read_text(errors="replace")
    flags, edition = headers(text)
    ok = 0
    for extra in DUMPS:
        with tempfile.TemporaryDirectory(dir=WORK) as d:
            emit = [] if any(e.startswith("--emit") for e in extra) else ["--emit=metadata"]
            code, _ = run([args.rustc, str(path), "--edition", edition or "2015", *emit, "--out-dir", d,
                           "-Zunstable-options", "-Ainternal_features", "-Aincomplete_features",
                           *flags, *extra], cwd=d, timeout=120)
            ok += code == 0
    return ok


def dumps():
    picked = json.loads(Path(args.list).read_text())
    tests = [t["test"] if isinstance(t, dict) else t for t in picked]
    with ThreadPoolExecutor(args.jobs) as ex:
        done = list(ex.map(dump, tests))
    print(f"dumps: {len(tests)} tests x {len(DUMPS)} options, {sum(done)} runs succeeded", flush=True)


only = args.only.split(",")
if "prints" in only:
    prints()
if "targets" in only:
    cross()
if "dumps" in only:
    dumps()
if "links" in only:
    links()

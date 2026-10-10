#!/usr/bin/env python3
"""Cross-target build and link: every target rustc knows must build `core` and `alloc` and link a
program with its documented linker, with no undefined symbols.

For each target, builds rustc/xlink-probe (a `no_std` program using 128-bit integers, float
conversions and math, float formatting, large copies and atomics, so that it needs the
compiler-builtins routines that go missing) with `cargo -Zbuild-std=core,alloc` and links it.
Targets whose default linker is a C compiler driver (absent for cross targets here) link with
`rust-lld` in the flavor their spec names instead. lld reports undefined symbols as errors, which
is what this looks for.

  link-undefined  the link fails with undefined symbols
  link            the link fails otherwise
  env             a library or startup file of the target's C sysroot is missing here
  build           core or alloc (or the probe) does not compile for the target
  ice             the compiler crashes

    rustc/xlink.py --toolchain nightly-2026-10-06 --work <dir> [--targets t1,t2] [--jobs 6]

Writes <work>/results.json. Target directories are removed after each build (about 100 MB each).
"""

import argparse
import json
import os
import re
import shutil
import subprocess
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--toolchain", required=True)
p.add_argument("--work", required=True)
p.add_argument("--targets")
p.add_argument("--jobs", type=int, default=6)
args = p.parse_args()
WORK = Path(args.work).resolve()
WORK.mkdir(parents=True, exist_ok=True)
PROBE = Path(__file__).resolve().parent / "xlink-probe"
# Targets that need more than a target name (a CPU, an external linker that is not lld).
SKIP = re.compile(r"^(amdgcn|nvptx|bpf|spirv)|-uefi-|avr-none")


def rustc(*a):
    return subprocess.run(["rustc", f"+{args.toolchain}", *a], capture_output=True, text=True,
                          env=dict(os.environ, RUSTC_BOOTSTRAP="1"))


def link_flags(spec):
    """RUSTFLAGS for linking the probe with lld, from the target's spec."""
    flavor = spec.get("linker-flavor", "")
    linker = spec.get("linker", "")
    own_lld = "lld" in linker or flavor.endswith("-lld") or flavor in ("wasm-lld", "wasm-lld-cc")
    flags = []
    if spec.get("is-like-wasm") or flavor.startswith("wasm"):
        return flags + ["-Clink-arg=--no-entry", "-Clink-arg=--export=probe_entry"]
    if flavor.startswith("msvc") or spec.get("is-like-msvc"):
        if not own_lld:
            flags += ["-Clinker=rust-lld", "-Clinker-flavor=lld-link"]
        return flags + ["-Clink-arg=/ENTRY:probe_entry", "-Clink-arg=/NODEFAULTLIB"]
    if flavor.startswith("darwin") or spec.get("is-like-darwin"):
        if not own_lld:
            flags += ["-Clinker=rust-lld", "-Clinker-flavor=ld64.lld"]
        return flags + ["-Clink-arg=-e", "-Clink-arg=_probe_entry", "-Clink-arg=-undefined",
                        "-Clink-arg=dynamic_lookup"]
    if not own_lld:
        flags += ["-Clinker=rust-lld", "-Clinker-flavor=ld.lld"]
    return flags + ["-Clink-arg=--entry=probe_entry"]


def one(target):
    if SKIP.search(target):
        return target, {"result": "skipped"}
    out = rustc("--print", "target-spec-json", "-Zunstable-options", "--target", target)
    if out.returncode != 0:
        return target, {"result": "skipped", "why": "no spec"}
    spec = json.loads(out.stdout)
    flags = link_flags(spec)
    tdir = WORK / "target" / target
    env = dict(os.environ, CARGO_TARGET_DIR=str(tdir), RUSTFLAGS=" ".join(flags), CARGO_TERM_COLOR="never")
    env.pop("RUSTC_WRAPPER", None)
    try:
        r = subprocess.run(["cargo", f"+{args.toolchain}", "build", "--release", "-Zbuild-std=core,alloc",
                            "-Zbuild-std-features=compiler-builtins-mem", "--target", target],
                           cwd=PROBE, env=env, capture_output=True, text=True, timeout=1500)
        code, err = r.returncode, r.stderr
    except subprocess.TimeoutExpired:
        code, err = "timeout", ""
    shutil.rmtree(tdir, ignore_errors=True)
    if code == 0:
        result = "ok"
    elif "internal compiler error" in err or "panicked at" in err:
        result = "ice"
    elif "linking with" in err or "rust-lld: error" in err or "lld: error" in err:
        if re.search(r"undefined (symbol|reference)", err):
            result = "link-undefined"
        elif re.search(r"unable to find library|cannot open crt|cannot open .*\.o\b|No such file", err):
            # Libraries or startup files that ship with the target's prebuilt std or a C sysroot.
            result = "env"
        else:
            result = "link"
    else:
        result = "build"
    undefined = sorted(set(re.findall(r"undefined symbol: ([^\s\n]+)", err)))[:20]
    first = next((l for l in err.splitlines() if re.search(r"error(\[|:)", l)), "")[:300]
    return target, {"result": result, "flags": flags, "undefined": undefined, "first": first,
                    "tail": err[-2500:] if code != 0 else ""}


def main():
    targets = args.targets.split(",") if args.targets else rustc("--print", "target-list").stdout.split()
    with ThreadPoolExecutor(args.jobs) as ex:
        results = dict(ex.map(one, targets))
    (WORK / "results.json").write_text(json.dumps(results, indent=1))
    print(Counter(r["result"] for r in results.values()))
    for t, r in sorted(results.items()):
        if r["result"] not in ("ok", "skipped"):
            print(f"{r['result']:15} {t:40} {' '.join(r['undefined'][:6]) or r['first'][:120]}")


main()

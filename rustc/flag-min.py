#!/usr/bin/env python3
"""Minimize the failures of a flag-walk.py run: for each distinct first error among rows
whose clean build with A failed, find the smallest set of the row's options that still gives
the same error (delta debugging, a clean build of the fixture per test).

    rustc/flag-min.py --rustc <rustc> --fixture fixtures/sink --walk <flag-walk work dir>
        [--per-error 1] [--jobs 4]

Prints one line per error: the minimal options. Writes <walk>/minimized.json.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--fixture", required=True)
p.add_argument("--walk", required=True)
p.add_argument("--per-error", type=int, default=1)
p.add_argument("--jobs", type=int, default=4)
p.add_argument("--toolchain", default="nightly-2026-10-06")
p.add_argument("--target", default="x86_64-unknown-linux-gnu")
args = p.parse_args()

FLAG_BASE = ("-Cunsafe-allow-abi-mismatch=sanitizer,sanitizer-cfi-normalize-integers,"
             "sanitizer-cfi-minimal-runtime,retpoline,retpoline-external-thunk,"
             "indirect-branch-cs-prefix,fixed-x18,reg-struct-return,regparm,branch-protection")
WALK = Path(args.walk)


def signature(line):
    """An error line with paths, symbols, hashes and quoted names removed."""
    line = re.sub(r"_R\w+|/\S+|`[^`]*`|\b[0-9a-f]{16}\b", "…", line)
    line = re.sub(r"\d+", "N", line)
    return line[:120]


def first_error(log):
    for l in log.splitlines():
        if l.startswith(("error", "rustc-LLVM ERROR", "LLVM ERROR")) and "could not compile" not in l:
            return l
        if "panicked at" in l:
            return l
    return ""


def minimize(flags, sig):
    work = Path(tempfile.mkdtemp(dir=WALK))
    src, target = work / "s", work / "t"
    shutil.copytree(Path(args.fixture), src, ignore=shutil.ignore_patterns("target", "edits", "edit"))
    e = dict(os.environ, RUSTC=args.rustc, RUSTC_WRAPPER="", CARGO_INCREMENTAL="1", CARGO_TERM_COLOR="never")

    def fails(fl):
        shutil.rmtree(target, ignore_errors=True)
        e["RUSTFLAGS"] = " ".join([FLAG_BASE] + fl)
        r = subprocess.run(["cargo", f"+{args.toolchain}", "build", "--workspace", "--offline", "-j", "4",
                            "--target", args.target, "--target-dir", str(target)],
                           cwd=src, env=e, capture_output=True, text=True)
        return r.returncode != 0 and signature(first_error(r.stderr)) == sig

    if not fails(flags):
        shutil.rmtree(work)
        return None
    n = 2
    while len(flags) >= 2:
        chunk = max(1, len(flags) // n)
        for i in range(0, len(flags), chunk):
            rest = flags[:i] + flags[i + chunk:]
            if fails(rest):
                flags, n = rest, max(n - 1, 2)
                break
        else:
            if chunk == 1:
                break
            n = min(len(flags), n * 2)
    shutil.rmtree(work)
    return flags


rows = [json.loads(l) for l in open(WALK / "results.jsonl")]
todo = {}
for r in rows:
    if r["A_ok"]:
        continue
    err = (r.get("errors") or [r.get("error", "")])[0]
    sig = signature(err)
    todo.setdefault(sig, [])
    if len(todo[sig]) < args.per_error:
        todo[sig].append(r["A"])

jobs = [(sig, fl) for sig, fls in todo.items() for fl in fls]
with ThreadPoolExecutor(args.jobs) as ex:
    found = list(ex.map(lambda j: (j[0], minimize(list(j[1]), j[0])), jobs))
out = [{"error": sig, "minimal": fl} for sig, fl in found]
(WALK / "minimized.json").write_text(json.dumps(out, indent=1))
for o in out:
    print(o["minimal"], "->", o["error"])

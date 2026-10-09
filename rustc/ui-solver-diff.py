#!/usr/bin/env python3
"""rustc's UI tests under nightly's default trait solver and under the one the test suite pins.

Nightly builds use the new trait solver everywhere by default (compiler-team MCP #1014,
rust-lang/rust#160895), but compiletest passes `-Znext-solver=coherence` to every UI test, so
the suite checks the old behaviour only. This compiles each test both ways, the way its `//@`
headers say (as rustc/ui-coverage.py does), and lists the tests whose outcome differs: an ICE,
success against failure, or a different first error.

    rustc/ui-solver-diff.py --rustc <rustc> --tests <rust>/tests/ui --out <dir> [--jobs 8]

Writes <out>/results.jsonl and prints the differences, ICEs first.
"""

import argparse
import json
import os
import re
import subprocess
import tempfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--tests", required=True)
p.add_argument("--out", required=True)
p.add_argument("--jobs", type=int, default=8)
args = p.parse_args()
OUT = Path(args.out)

SKIP = re.compile(r"^//@\s*(aux-build|aux-crate|aux-bin|proc-macro|add-minicore|needs-llvm-components|"
                  r"only-(?!x86_64|linux|unix|64bit)|ignore-x86_64|ignore-linux|needs-sanitizer|needs-profiler|"
                  r"known-bug)|-Znext-solver", re.M)
NO_BUILD = ("check-pass", "check-fail")


def headers(text):
    flags, edition, revision, kind = [], None, None, None
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
        elif key in ("check-pass", "build-pass", "run-pass", "check-fail", "build-fail", "run-fail"):
            kind = key
    if revision:
        flags += ["--cfg", revision]
    return flags, edition, kind


def compile(path, flags, edition, kind, solver):
    with tempfile.TemporaryDirectory(dir=OUT / "scratch") as d:
        emit = "--emit=metadata" if kind in NO_BUILD or kind is None else "--emit=link"
        argv = [args.rustc, str(path), "--edition", edition or "2015", emit, "--out-dir", d,
                "-Zunstable-options", "-Ainternal_features", "-Aincomplete_features", *solver, *flags]
        try:
            r = subprocess.run(argv, capture_output=True, text=True, timeout=120, cwd=d,
                               env=dict(os.environ, RUSTC_BOOTSTRAP="1"))
        except subprocess.TimeoutExpired:
            return {"status": "timeout", "error": ""}
        ice = ("internal compiler error" in r.stderr or "the compiler unexpectedly panicked" in r.stderr
               or "rustc interrupted by SIG" in r.stderr)
        first = next((l for l in r.stderr.splitlines() if l.startswith("error")), "")
        first = re.sub(r"/\S+|`[^`]*`", "…", first)[:160]
        return {"status": "ok" if r.returncode == 0 else ("ice" if ice else "error"), "error": first}


def one(path):
    text = path.read_text(errors="replace")
    rel = str(path.relative_to(args.tests))
    if SKIP.search(text):
        return {"test": rel, "skipped": True}
    flags, edition, kind = headers(text)
    default = compile(path, flags, edition, kind, [])
    pinned = compile(path, flags, edition, kind, ["-Znext-solver=coherence"])
    return {"test": rel, "kind": kind, "default": default, "pinned": pinned}


OUT.mkdir(parents=True, exist_ok=True)
(OUT / "scratch").mkdir(exist_ok=True)
tests = sorted(p for p in Path(args.tests).rglob("*.rs") if "auxiliary" not in p.parts)
results = []
with ThreadPoolExecutor(args.jobs) as ex, (OUT / "results.jsonl").open("w") as out:
    for res in ex.map(one, tests):
        out.write(json.dumps(res) + "\n")
        results.append(res)

ran = [r for r in results if not r.get("skipped")]
differ = [r for r in ran if r["default"]["status"] != r["pinned"]["status"]
          or r["default"]["error"] != r["pinned"]["error"]]
print(f"{len(ran)} tests compiled both ways; {len(differ)} differ")
print(Counter((r["pinned"]["status"], r["default"]["status"]) for r in differ).most_common())
for r in sorted(differ, key=lambda r: (r["default"]["status"] != "ice", r["test"])):
    print(f"{r['pinned']['status']:>7} -> {r['default']['status']:<7} {r['test']}  {r['default']['error'][:100]}")

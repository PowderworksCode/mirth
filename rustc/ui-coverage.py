#!/usr/bin/env python3
"""Which compiler functions each of rustc's UI tests reaches that a baseline (the fixture's
builds) does not, with a coverage-instrumented rustc (rustc/coverage.toml); then a small set
of tests that reaches the most of them.

    rustc/ui-coverage.py run --rustc <instrumented rustc> --tests <rust>/tests/ui
        --sites <build>/mirth-sites --baseline <logs dir> [--baseline ...] --out <dir> [--jobs 6]
    rustc/ui-coverage.py pick --out <dir> [--count 200]

`run` compiles each test file the way its `//@` headers say, as far as one rustc call can:
`compile-flags`, `edition`, the first of `revisions` (as `--cfg` with its own flags), metadata
only for tests that do not build (check-pass, and tests expected to fail before codegen), a
full build otherwise. Tests that need auxiliary crates, proc macros, another target or
`minicore` are skipped. Writes <out>/tests.jsonl: per test, whether it compiled and the
indices (into <out>/functions.json) of the functions it reached beyond the baseline.

`pick` chooses tests greedily, each adding the most functions not yet reached, and writes
<out>/picked.json.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

p = argparse.ArgumentParser()
sub = p.add_subparsers(dest="cmd", required=True)
r = sub.add_parser("run")
r.add_argument("--rustc", required=True)
r.add_argument("--tests", required=True)
r.add_argument("--sites", required=True)
r.add_argument("--baseline", action="append", default=[])
r.add_argument("--out", required=True)
r.add_argument("--jobs", type=int, default=6)
r.add_argument("--limit", type=int, default=0)
k = sub.add_parser("pick")
k.add_argument("--out", required=True)
k.add_argument("--count", type=int, default=200)
args = p.parse_args()
OUT = Path(args.out)

SKIP = re.compile(r"^//@\s*(aux-build|aux-crate|aux-bin|proc-macro|add-minicore|needs-llvm-components|"
                  r"only-(?!x86_64|linux|unix|64bit)|ignore-x86_64|ignore-linux|needs-sanitizer|needs-profiler|"
                  r"needs-asm-support|known-bug)", re.M)
NO_BUILD = ("check-pass", "check-fail")


def hits(directory):
    out = set()
    for log in Path(directory).glob("*.log"):
        for line in log.read_text(errors="replace").splitlines():
            if line.startswith("V\t"):
                out.add(line[2:])
    return out


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


def run_one(path):
    text = path.read_text(errors="replace")
    rel = str(path.relative_to(args.tests))
    if SKIP.search(text):
        return {"test": rel, "status": "skipped"}
    flags, edition, kind = headers(text)
    with tempfile.TemporaryDirectory(dir=OUT / "scratch") as d:
        emit = "--emit=metadata" if kind in NO_BUILD or kind is None else "--emit=link"
        argv = [args.rustc, str(path), "--edition", edition or "2015", emit, "--out-dir", d,
                "-Zunstable-options", "-Ainternal_features", *flags]
        env = dict(os.environ, MIRTH_OUT=d + "/logs", RUSTC_BOOTSTRAP="1")
        try:
            done = subprocess.run(argv, capture_output=True, text=True, timeout=120, env=env, cwd=d)
            ice = ("internal compiler error" in done.stderr or "the compiler unexpectedly panicked" in done.stderr
                   or "rustc interrupted by SIG" in done.stderr)
            status = "ok" if done.returncode == 0 else ("ice" if ice else "error")
            first = next((l for l in done.stderr.splitlines() if l.startswith("error")), "")[:160]
        except subprocess.TimeoutExpired:
            status, first = "timeout", ""
        new = sorted(index[s] for s in hits(d + "/logs") - baseline if s in index)
    return {"test": rel, "status": status, "kind": kind, "error": first, "new": new}


if args.cmd == "run":
    functions = []
    for table in Path(args.sites).glob("*.sites"):
        for line in table.read_text(errors="replace").splitlines():
            f = line.split("\t")
            if len(f) >= 7 and f[1] == "cover":
                functions.append((f[0], f[4], f[6]))
    functions.sort()
    index = {site: i for i, (site, _, _) in enumerate(functions)}
    baseline = set()
    for b in args.baseline:
        for sub_dir in [Path(b), *Path(b).glob("*")]:
            if sub_dir.is_dir():
                baseline |= hits(sub_dir)
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "scratch").mkdir(exist_ok=True)
    (OUT / "functions.json").write_text(json.dumps(
        {"functions": [[path, span] for _, path, span in functions],
         "baseline": sorted(index[s] for s in baseline if s in index)}))
    done = set()
    results = OUT / "tests.jsonl"
    if results.exists():
        done = {json.loads(l)["test"] for l in results.read_text().splitlines()}
    tests = sorted(p for p in Path(args.tests).rglob("*.rs")
                   if "auxiliary" not in p.parts and str(p.relative_to(args.tests)) not in done)
    if args.limit:
        tests = tests[:args.limit]
    print(f"{len(functions)} functions, {len(baseline & index.keys())} in the baseline; "
          f"{len(tests)} tests to run", flush=True)
    with ThreadPoolExecutor(args.jobs) as ex, results.open("a") as out:
        for n, res in enumerate(ex.map(run_one, tests)):
            out.write(json.dumps(res) + "\n")
            if n % 500 == 0:
                out.flush()
                print(f"{n} tests", flush=True)
    shutil.rmtree(OUT / "scratch", ignore_errors=True)

if args.cmd == "pick":
    info = json.loads((OUT / "functions.json").read_text())
    tests = [json.loads(l) for l in (OUT / "tests.jsonl").read_text().splitlines()]
    tests = [t for t in tests if t.get("new")]
    reached = set()
    for t in tests:
        reached |= set(t["new"])
    covered, picked = set(), []
    sets = {t["test"]: set(t["new"]) for t in tests}
    status = {t["test"]: t["status"] for t in tests}
    while len(picked) < args.count and sets:
        best = max(sets, key=lambda name: len(sets[name] - covered))
        gain = sets[best] - covered
        if not gain:
            break
        covered |= gain
        picked.append({"test": best, "status": status[best], "adds": len(gain), "total": len(covered)})
        del sets[best]
    total = len(info["functions"])
    base = len(info["baseline"])
    print(f"baseline {base} of {total} functions ({100 * base / total:.1f}%); all tests reach "
          f"{len(reached)} more; {len(picked)} picked tests reach {len(covered)} more "
          f"({100 * (base + len(covered)) / total:.1f}% in all)")
    for t in picked[:40]:
        print(f"  +{t['adds']:5} {t['total']:6}  {t['status']:7} {t['test']}")
    (OUT / "picked.json").write_text(json.dumps(picked, indent=1))

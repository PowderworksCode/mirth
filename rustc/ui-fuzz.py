#!/usr/bin/env python3
"""Incremental rebuilds of rustc's UI tests, most of which fail to compile on purpose: the
fuzzer's edits applied to each test file, each rebuild compared with a clean build of the same
source, so that error reporting and recovery are exercised under incremental compilation.

    rustc/ui-fuzz.py --rustc <rustc> --tests <rust>/tests/ui --list picked.json --work <dir>
        [--edits 20] [--jobs 8] [--pause-on-finding]

For each test in the list (rustc/ui-coverage.py pick), compiled the way its `//@` headers say:
build it incrementally, then repeatedly apply a random edit (mutations.py) and rebuild it
incrementally, build the edited file again with a fresh incremental directory (same file, same
working directory), and compare:

  status  both succeed, both fail, or both crash
  diag    the diagnostics, with paths and the incremental directory taken out
  output  the .rmeta and .rlib (normalized as artifacts.py does) when both succeed
  ice     both crash or neither does

A clean build is made again before a difference counts (nondeterminism). Findings go to
<work>/findings/<test>-<n>/ with the source, both outputs and the edit history.
"""

import argparse
import difflib
import hashlib
import json
import os
import random
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

here = Path(__file__).parent
sys.path.insert(0, str(here))
import artifacts  # noqa: E402
import mutations  # noqa: E402

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--tests", required=True)
p.add_argument("--list", required=True)
p.add_argument("--work", required=True)
p.add_argument("--edits", type=int, default=20)
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--flags", default="", help="extra rustc options for every build")
p.add_argument("--pause-on-finding", action="store_true")
args = p.parse_args()
WORK = Path(args.work).resolve()
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


def build(directory, source, flags, edition, kind, incremental):
    """Compile `source` (a file in `directory`) with outputs in `directory/out`."""
    out = directory / "out"
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    emit = "--emit=metadata" if kind in NO_BUILD or kind is None else "--emit=link,metadata"
    argv = [args.rustc, source.name, "--edition", edition or "2015", emit, "--out-dir", "out",
            "-Zunstable-options", "-Ainternal_features", "-Aincomplete_features", "--error-format=short",
            *flags, *args.flags.split()]
    if incremental:
        argv.append(f"-Cincremental={incremental}")
    try:
        r = subprocess.run(argv, capture_output=True, text=True, timeout=300, cwd=directory,
                           env=dict(os.environ, RUSTC_BOOTSTRAP="1", RUST_BACKTRACE="0"))
        code, err = r.returncode, r.stderr
    except subprocess.TimeoutExpired:
        code, err = -1, "timeout"
    ice = ("internal compiler error" in err or "the compiler unexpectedly panicked" in err
           or "rustc interrupted by SIG" in err)
    diag = sorted(set(re.sub(r"\(\d+\)", "(…)", l) for l in err.splitlines()
                      if l and not l.startswith(("note: ", "  ", "query stack", "#"))))
    files = {}
    for f in sorted(out.iterdir()):
        if f.suffix == ".rmeta":
            files[f.name] = hashlib.sha256(f.read_bytes()).hexdigest()
        elif f.suffix == ".rlib":
            files[f.name] = artifacts.normalized_rlib(f)
    return {"code": code, "ice": ice, "diag": diag, "files": files, "stderr": err}


def compare(inc, clean):
    found = []
    # Some tests crash the compiler on purpose; only a crash on one side counts.
    if inc["ice"] != clean["ice"]:
        found.append("ICE " + ("incremental" if inc["ice"] else "clean") + " only")
    if (inc["code"] == 0) != (clean["code"] == 0):
        found.append(f"status: incremental {inc['code']}, clean {clean['code']}")
    if inc["diag"] != clean["diag"]:
        only_inc = [d for d in inc["diag"] if d not in clean["diag"]][:3]
        only_clean = [d for d in clean["diag"] if d not in inc["diag"]][:3]
        found.append(f"diag: incremental only {only_inc}; clean only {only_clean}")
    if inc["code"] == 0 and clean["code"] == 0 and inc["files"] != clean["files"]:
        found.append("output: " + ", ".join(k for k in set(inc["files"]) | set(clean["files"])
                                            if inc["files"].get(k) != clean["files"].get(k)))
    return found


def fuzz(test):
    if (WORK / "PAUSED").exists():
        return test, "not run"
    path = Path(args.tests) / test
    text = path.read_text(errors="replace")
    flags, edition, kind = headers(text)
    name = re.sub(r"\W", "_", test)
    home = WORK / "w" / name
    shutil.rmtree(home, ignore_errors=True)
    # The clean build uses the same directory and file, with a fresh incremental directory, so
    # that nothing but incremental state tells the two builds apart.
    inc_dir = clean_dir = home / "src"
    inc_dir.mkdir(parents=True)
    src_inc = src_clean = inc_dir / path.name
    src_inc.write_text(text)
    rng = random.Random(test)
    history = []
    build(inc_dir, src_inc, flags, edition, kind, str(home / "incr"))
    found_any = 0
    for n in range(args.edits):
        old = src_inc.read_text()
        fn = rng.choices([e for e, _ in mutations.EDITS], weights=[w for _, w in mutations.EDITS])[0]
        new = fn(old, rng, n)
        if new is None or new == old:
            continue
        src_inc.write_text(new)
        history.append({"edit": fn.__name__, "diff": "".join(difflib.unified_diff(
            old.splitlines(True), new.splitlines(True), "a", "b"))})
        inc = build(inc_dir, src_inc, flags, edition, kind, str(home / "incr"))
        shutil.rmtree(home / "incr-clean", ignore_errors=True)
        clean = build(clean_dir, src_clean, flags, edition, kind, str(home / "incr-clean"))
        found = compare(inc, clean)
        if found and not any(f.startswith("ICE") for f in found):
            shutil.rmtree(home / "incr-clean", ignore_errors=True)
            again = build(clean_dir, src_clean, flags, edition, kind, str(home / "incr-clean"))
            if compare(clean, again):
                found = ["P5 " + f for f in found]
        if found:
            found_any += 1
            d = WORK / "findings" / f"{name}-{n}"
            d.mkdir(parents=True, exist_ok=True)
            (d / path.name).write_text(new)
            (d / "finding.json").write_text(json.dumps({"test": test, "found": found, "flags": flags,
                                                       "edition": edition, "kind": kind,
                                                       "history": history}, indent=1))
            (d / "inc.stderr").write_text(inc["stderr"])
            (d / "clean.stderr").write_text(clean["stderr"])
            if args.pause_on_finding and not all(f.startswith("P5") for f in found):
                (WORK / "PAUSED").write_text(json.dumps({"test": test, "edit": n, "found": found}, indent=1))
                break
    shutil.rmtree(home, ignore_errors=True)
    return test, f"{len(history)} edits, {found_any} findings"


WORK.mkdir(parents=True, exist_ok=True)
(WORK / "PAUSED").unlink(missing_ok=True)
picked = json.loads(Path(args.list).read_text())
tests = [t["test"] if isinstance(t, dict) else t for t in picked]
done_path = WORK / "done.txt"
done = set(done_path.read_text().split()) if done_path.exists() else set()
with ThreadPoolExecutor(args.jobs) as ex, done_path.open("a") as log:
    for test, result in ex.map(fuzz, [t for t in tests if t not in done]):
        print(f"{test}: {result}", flush=True)
        if result != "not run" and not (WORK / "PAUSED").exists():
            log.write(test + "\n")
            log.flush()

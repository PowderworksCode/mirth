#!/usr/bin/env python3
"""Solver differential: the trait solvers and the borrow checkers must agree.

Compiles each standalone UI test four ways: the old trait solver (`-Znext-solver=coherence`,
what compiletest pins), nightly's default (the new solver everywhere), the old solver with
Polonius (`-Zpolonius=next`), and the new solver with Polonius. Compared with the old solver
and NLL:

  ice      a configuration crashes where the reference does not
  verdict  accepted by one and rejected by the other
  codes    both reject, with different sets of error codes (reported, not a finding: the
           solvers word errors differently)

A program accepted only by a non-reference configuration and runnable (it has `fn main`) is
interpreted with Miri under that configuration's flags: undefined behavior there means the
other configuration accepted something unsound (the Polonius soundness bugs had that shape).

Tests that name a solver or Polonius in their headers (they test the difference on purpose) are
left out.

    rustc/solver-diff.py --rustc <rustc> --tests <rust>/tests/ui --work <dir> [--only <substr>]
        [--known <file>] [--jobs 8] [--pause-on-finding] [--recheck]
"""

import argparse
import json
import re
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import uitest  # noqa: E402

CONFIGS = {
    "old": ["-Znext-solver=coherence"],
    "next": [],
    "old-polonius": ["-Znext-solver=coherence", "-Zpolonius=next"],
    "next-polonius": ["-Zpolonius=next"],
}
REFERENCE = "old"
KINDS = ("check-pass", "build-pass", "run-pass", "check-fail", "build-fail", "run-fail", None)

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--tests", required=True)
p.add_argument("--work", required=True)
p.add_argument("--only")
p.add_argument("--known")
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--pause-on-finding", action="store_true")
p.add_argument("--recheck", action="store_true")
args = p.parse_args()
WORK = Path(args.work).resolve()
(WORK / "scratch").mkdir(parents=True, exist_ok=True)
known = set(Path(args.known).read_text().split()) if args.known else set()


def codes(stderr):
    return sorted(set(re.findall(r"error\[(E\d{4})\]", stderr)))


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    record = {"test": rel, "kind": kind}
    results = {}
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        for name, cfg in CONFIGS.items():
            # Metadata is enough for the verdict: type checking and borrow checking run for it.
            status, stderr, _ = uitest.compile(args.rustc, path.resolve(), d / name, flags, edition, cfg,
                                               timeout=120, emit="metadata")
            results[name] = {"status": status, "codes": codes(stderr), "stderr": stderr[-2500:]}
    record["status"] = {k: v["status"] for k, v in results.items()}
    ref = results[REFERENCE]
    found, notes = [], []
    for name, r in results.items():
        if name == REFERENCE:
            continue
        if r["status"] == "ice" and ref["status"] != "ice":
            found.append({"config": name, "what": "ice", "stderr": r["stderr"]})
        elif r["status"] == "timeout" and ref["status"] != "timeout":
            found.append({"config": name, "what": "timeout"})
        elif {r["status"], ref["status"]} == {"ok", "error"}:
            f = {"config": name, "what": f"verdict: {REFERENCE} {ref['status']}, {name} {r['status']}",
                 "ref_codes": ref["codes"], "codes": r["codes"],
                 "stderr": (r if r["status"] == "error" else ref)["stderr"]}
            accepted_by = name if r["status"] == "ok" else REFERENCE
            if "fn main" in path.read_text(errors="replace"):
                m = uitest.miri(path.resolve(), flags, edition, CONFIGS[accepted_by], timeout=120, cwd=WORK / "scratch")
                f["miri"] = {"config": accepted_by, "status": m["status"], "stderr": m["stderr"][-1500:]}
                if m["status"] == "ub":
                    f["what"] += f"; Miri: UB under {accepted_by}"
            found.append(f)
        elif r["status"] == ref["status"] == "error" and r["codes"] != ref["codes"]:
            notes.append(f"{name} codes {r['codes']} vs {ref['codes']}")
    record["found"] = [f"{f['config']}: {f['what']}" for f in found]
    record["notes"] = notes
    if found:
        outdir = WORK / "findings" / rel.replace("/", "__")
        shutil.rmtree(outdir, ignore_errors=True)
        outdir.mkdir(parents=True)
        shutil.copy(path, outdir / path.name)
        (outdir / "finding.json").write_text(json.dumps(
            {"test": rel, "flags": flags, "edition": edition, "configs": CONFIGS, "found": found}, indent=1))
    return record, found


def main():
    wanted = None
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    skip = lambda text, flags: re.search(r"next-solver|polonius|^//@\s*revisions:.*\bnext\b", text, re.M)
    todo = []
    for path, flags, edition, kind in uitest.tests(args.tests, KINDS, skip):
        rel = str(path.relative_to(args.tests))
        if rel in known or (args.only and args.only not in rel) or (wanted is not None and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    print(f"{len(todo)} tests, configurations: {', '.join(CONFIGS)}", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()

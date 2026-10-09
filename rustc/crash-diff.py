#!/usr/bin/env python3
"""Internal checks on: what the compiler's own invariants say about every UI test.

Compiles each standalone UI test with the compiler under test and again with a second compiler
built from the same source with debug assertions (`rust.debug-assertions`), with
`-Zvalidate-mir` added. Findings are a crash, a failed assertion or a MIR validation error
under the second that the first does not have: rustc's invariants failing where release
builds go on silently (19 of the last 1,000 ICE reports needed such a build).

    rustc/crash-diff.py --rustc <release rustc> --checked <debug-assertions rustc>
        --tests <rust>/tests/ui --work <dir> [--extra "-Zvalidate-mir"] [--only <substr>]
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

KINDS = ("check-pass", "build-pass", "run-pass", "check-fail", "build-fail", "run-fail", None)

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--checked", required=True)
p.add_argument("--extra", default="-Zvalidate-mir")
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


def message(stderr):
    """The first line saying what went wrong inside the compiler."""
    for pattern in (r"panicked at [^\n]*\n[^\n]*", r"internal compiler error: [^\n]*", r"broken MIR[^\n]*"):
        m = re.search(pattern, stderr)
        if m:
            return re.sub(r"/\S+/compiler/", "compiler/", m.group(0))[:400]
    return ""


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    emit = "metadata" if kind in ("check-pass", "check-fail", None) else "link"
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        a, ea, _ = uitest.compile(args.rustc, path.resolve(), d / "release", flags, edition, timeout=300, emit=emit)
        b, eb, _ = uitest.compile(args.checked, path.resolve(), d / "checked", flags, edition, args.extra.split(),
                                  timeout=600, emit=emit)
    record = {"test": rel, "release": a, "checked": b}
    found = []
    if b == "ice" and a != "ice":
        found.append({"what": "only with internal checks", "message": message(eb), "stderr": eb[-4000:]})
    elif b == "ice" and a == "ice" and message(ea) != message(eb):
        record["note"] = "both crash, differently"
    record["found"] = [f"{f['what']}: {f['message'][:160]}" for f in found]
    if found:
        out = WORK / "findings" / rel.replace("/", "__")
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        shutil.copy(path, out / path.name)
        (out / "finding.json").write_text(json.dumps({"test": rel, "flags": flags, "edition": edition,
                                                       "extra": args.extra, "found": found}, indent=1))
    return record, found


def main():
    wanted = None
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    todo = []
    for path, flags, edition, kind in uitest.tests(args.tests, KINDS):
        rel = str(path.relative_to(args.tests))
        if rel in known or (args.only and args.only not in rel) or (wanted is not None and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    print(f"{len(todo)} tests", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()

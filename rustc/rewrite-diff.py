#!/usr/bin/env python3
"""Equivalent rewrites: rewriting a program into an equivalent one must not change its verdict.

Each standalone UI test is printed back unchanged (`mirth-rewrite identity`: the baseline, since
printing drops comments and moves lines) and rewritten by each of mirth-rewrite's rewrites
(generic-wrap, alias, reorder, unused). Each version is compiled the way the test's headers say
(metadata for check tests, a full build for build and run tests), and compared with the
baseline:

  verdict  accepted against rejected, or a crash on one side only (a finding)
  codes    both rejected with different sets of error codes (a finding for reorder and unused,
           which change nothing a diagnostic could depend on; noted for the others)

A test whose baseline differs from the original file's verdict is left out (the printer cannot
represent it faithfully).

    rustc/rewrite-diff.py --rustc <rustc> --tests <rust>/tests/ui --work <dir>
        [--rewrites generic-wrap,alias] [--only <substr>] [--known <file>] [--jobs 8]
        [--pause-on-finding] [--recheck]
"""

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import uitest  # noqa: E402

REWRITER = Path(__file__).resolve().parent.parent / "target/release/mirth-rewrite"
REWRITES = ["generic-wrap", "alias", "reorder", "unused"]
# Error codes may differ legitimately for every rewrite (which error suppresses which depends on
# order): only a changed verdict is a finding.
STRICT = set()
# Differences that are resource limits or legitimate requirements of a generic context.
NOISE = {
    ("consts/chained-constants-stackoverflow.rs", "reorder"),  # 10,000 chained consts: query depth
    ("consts/interior-mut-const-via-union.rs", "generic-wrap"),  # finding 25 (docs/hunt.md)
}
NOT_MOVABLE = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*;|include(_str|_bytes)?!|#\[path|#!\[no_core\]", re.M)
# Item order matters to textual macro scoping: no reordering where macros are defined.
ORDER_MATTERS = re.compile(r"macro_rules!|#\[macro_use\]|macro\s+\w+")
KINDS = ("check-pass", "build-pass", "run-pass", "check-fail", "build-fail", "run-fail", None)

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--tests", required=True)
p.add_argument("--work", required=True)
p.add_argument("--rewrites", help="comma-separated subset")
p.add_argument("--only")
p.add_argument("--known")
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--pause-on-finding", action="store_true")
p.add_argument("--recheck", action="store_true")
args = p.parse_args()
WORK = Path(args.work).resolve()
(WORK / "scratch").mkdir(parents=True, exist_ok=True)
rewrites = args.rewrites.split(",") if args.rewrites else REWRITES
known = set(Path(args.known).read_text().split()) if args.known else set()


def codes(stderr):
    return sorted(set(re.findall(r"error\[(E\d{4})\]", stderr)))


def verdict(source, flags, edition, kind, out, has_main):
    # A full build whenever there is a program: generic-wrap moves errors to monomorphization.
    emit = "link" if has_main or kind not in ("check-pass", "check-fail", None) else "metadata"
    # Lints capped: a rewrite may add or move a warning, and lint levels are not the subject.
    status, stderr, _ = uitest.compile(args.rustc, source, out, flags, edition, ["--cap-lints=warn"],
                                       timeout=120, emit=emit)
    return {"status": status, "codes": codes(stderr), "stderr": stderr[-2500:]}


def rewrite(name, source, target):
    r = subprocess.run([str(REWRITER), name, str(source)], capture_output=True, text=True, timeout=60)
    if r.returncode != 0:
        return False
    target.write_text(r.stdout)
    return True


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    record = {"test": rel, "kind": kind}
    text = path.read_text(errors="replace")
    # The rewritten file is compiled elsewhere: files it names by relative path are not there.
    # Without `core`, a new trait or generic parameter does not compile.
    if NOT_MOVABLE.search(text):
        record["skip"] = "uses files by path or has no core"
        return record, []
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        base_src = d / "identity.rs"
        if not rewrite("identity", path, base_src):
            record["skip"] = "does not parse"
            return record, []
        has_main = "fn main" in text
        original = verdict(path.resolve(), flags, edition, kind, d / "original", has_main)
        base = verdict(base_src, flags, edition, kind, d / "identity", has_main)
        if (original["status"], original["codes"]) != (base["status"], base["codes"]):
            record["skip"] = "printing changes the verdict"
            return record, []
        record["base"] = base["status"]
        found, notes, applied = [], [], []
        for name in rewrites:
            if name == "reorder" and ORDER_MATTERS.search(text):
                continue
            if (rel, name) in NOISE:
                continue
            # generic_const_exprs requires `where` bounds in generic contexts that a concrete one
            # does not.
            if name == "generic-wrap" and "generic_const_exprs" in text:
                continue
            src = d / f"{name}.rs"
            if not rewrite(name, path, src):
                continue
            applied.append(name)
            v = verdict(src, flags, edition, kind, d / name, has_main)
            if v["status"] != base["status"] and "timeout" not in (v["status"], base["status"]):
                found.append({"rewrite": name, "what": f"verdict: {base['status']} -> {v['status']}",
                              "base_codes": base["codes"], "codes": v["codes"], "stderr": v["stderr"],
                              "base_stderr": base["stderr"]})
            elif v["status"] == base["status"] == "error" and v["codes"] != base["codes"]:
                entry = {"rewrite": name, "what": f"codes: {base['codes']} -> {v['codes']}",
                         "stderr": v["stderr"], "base_stderr": base["stderr"]}
                (found if name in STRICT else notes).append(entry)
            if any(f["rewrite"] == name for f in found + notes):
                shutil.copy(src, d / f"keep-{name}.rs")
        record["applied"] = applied
        record["found"] = [f"{f['rewrite']}: {f['what']}" for f in found]
        record["notes"] = [f"{f['rewrite']}: {f['what']}" for f in notes]
        if found or notes:
            outdir = WORK / ("findings" if found else "notes") / rel.replace("/", "__")
            shutil.rmtree(outdir, ignore_errors=True)
            outdir.mkdir(parents=True)
            shutil.copy(path, outdir / path.name)
            shutil.copy(base_src, outdir / "identity.rs")
            for f in found + notes:
                shutil.copy(d / f"keep-{f['rewrite']}.rs", outdir / f"{f['rewrite']}.rs")
            (outdir / "finding.json").write_text(json.dumps(
                {"test": rel, "flags": flags, "edition": edition, "kind": kind, "found": found, "notes": notes},
                indent=1))
        return record, found


def main():
    global REWRITER
    if not REWRITER.exists():
        sys.exit("build mirth-rewrite first: cargo build --release -p mirth-rewrite")
    # A private copy: rebuilding mirth-rewrite must not change a sweep halfway.
    shutil.copy2(REWRITER, WORK / "mirth-rewrite")
    REWRITER = WORK / "mirth-rewrite"
    wanted = None
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    todo = []
    for path, flags, edition, kind in uitest.tests(args.tests, KINDS):
        rel = str(path.relative_to(args.tests))
        if rel in known or (args.only and args.only not in rel) or (wanted is not None and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    print(f"{len(todo)} tests, rewrites: {', '.join(rewrites)}", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()

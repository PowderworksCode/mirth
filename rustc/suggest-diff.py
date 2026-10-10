#!/usr/bin/env python3
"""Suggestions apply: a machine-applicable suggestion must produce code that compiles the way the
suggestion promises.

For each standalone UI test, collects the diagnostics rustc emits (`--error-format=json`) and, for
each one carrying a `MachineApplicable` suggestion inside the test file, applies that one
suggestion (all its parts) to a copy and compiles the copy again. Findings:

  lint-breaks  the suggestion came from a warning (a lint) and the fixed file has an error the
               original did not: a lint's fix must never break a build
  parse        the fixed file no longer parses
  not-fixed    the same diagnostic (code and message) is reported again at the edited place
  ice          the fixed file crashes the compiler

Errors appearing after an *error's* suggestion is applied are expected (compilation gets further)
and are not reported. Tests with `//@ run-rustfix` are skipped by default: compiletest already
checks their fixes.

    rustc/suggest-diff.py --rustc <rustc> --tests <rust>/tests/ui --work <dir> [--with-rustfix]
        [--max 8] [--only <substr>] [--known <file>] [--jobs 8] [--pause-on-finding] [--recheck]
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import uitest  # noqa: E402

KINDS = ("check-pass", "build-pass", "run-pass", "check-fail", "build-fail", "run-fail", None)

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--tests", required=True)
p.add_argument("--work", required=True)
p.add_argument("--with-rustfix", action="store_true")
p.add_argument("--max", type=int, default=8, help="suggestions tried per test")
p.add_argument("--only")
p.add_argument("--known")
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--pause-on-finding", action="store_true")
p.add_argument("--recheck", action="store_true")
args = p.parse_args()
WORK = Path(args.work).resolve()
(WORK / "scratch").mkdir(parents=True, exist_ok=True)
known = set(Path(args.known).read_text().split()) if args.known else set()


def diagnostics(source, flags, edition, out):
    """(status, [diagnostic]) for `source`, metadata only."""
    out.mkdir(parents=True, exist_ok=True)
    argv = [args.rustc, str(source), "--edition", edition or "2015", "--emit=metadata", "-o", str(out / "x"),
            "-Zunstable-options", "-Ainternal_features", "-Aincomplete_features", "--error-format=json",
            *flags]
    try:
        r = subprocess.run(argv, capture_output=True, text=True, timeout=120, cwd=out,
                           env=dict(os.environ, RUSTC_BOOTSTRAP="1", RUST_BACKTRACE="0"))
    except subprocess.TimeoutExpired:
        return "timeout", []
    diags = []
    for line in r.stderr.splitlines():
        try:
            diags.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    if uitest.is_ice(r.stderr):
        return "ice", diags
    return ("ok" if r.returncode == 0 else "error"), diags


def suggestions(diag, file_name):
    """The machine-applicable suggestions of a diagnostic, each a list of (start, end, text)."""
    out = []
    for child in [diag] + diag.get("children", []):
        parts = [(s["byte_start"], s["byte_end"], s["suggested_replacement"]) for s in child.get("spans", [])
                 if s.get("suggested_replacement") is not None
                 and s.get("suggestion_applicability") == "MachineApplicable"
                 and Path(s["file_name"]).name == file_name]
        if parts:
            out.append(sorted(parts))
    return out


def key(diag):
    code = (diag.get("code") or {}).get("code") or ""
    return code, diag["message"]


def primary(diag, file_name):
    for s in diag.get("spans", []):
        if s.get("is_primary") and Path(s["file_name"]).name == file_name:
            return s["byte_start"], s["byte_end"]
    return None


def errors(diags):
    """The errors, by code (or lint name) when they have one: a renamed identifier changes the
    message of the same lint."""
    return {(key(d)[0], "" if key(d)[0] else key(d)[1]) for d in diags
            if d.get("level") == "error" and not d["message"].startswith("aborting")}


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    record = {"test": rel}
    # The copy is compiled elsewhere: files named by relative path would be missing.
    if re.search(r"^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*;|include(_str|_bytes)?!|#\[path",
                 path.read_text(errors="replace"), re.M):
        record["skip"] = "uses files by path"
        return record, []
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        src = d / path.name
        shutil.copy(path, src)
        status, diags = diagnostics(src, flags, edition, d / "orig")
        if status in ("ice", "timeout"):
            record["skip"] = f"original {status}"
            return record, []
        base_errors = errors(diags)
        text = src.read_bytes()
        found, tried = [], 0
        for diag in diags:
            for parts in suggestions(diag, path.name):
                if tried >= args.max:
                    break
                tried += 1
                # Apply from the end, so earlier offsets stay valid; overlapping parts are skipped.
                fixed, last = bytearray(text), None
                ok = True
                for start, end, repl in sorted(parts, reverse=True):
                    if last is not None and end > last:
                        ok = False
                        break
                    fixed[start:end] = repl.encode()
                    last = start
                if not ok:
                    continue
                fsrc = d / f"fix{tried}" / path.name
                fsrc.parent.mkdir()
                fsrc.write_bytes(bytes(fixed))
                fstatus, fdiags = diagnostics(fsrc, flags, edition, fsrc.parent)
                what = None
                if fstatus == "ice":
                    what = "ice"
                elif any("expected" in m or "unexpected" in m or "unknown start of token" in m
                         for _, m in errors(fdiags) - base_errors):
                    what = "parse"
                elif diag.get("level") == "warning" and errors(fdiags) - base_errors:
                    what = "lint-breaks"
                elif sum(key(fd) == key(diag) for fd in fdiags) >= sum(key(od) == key(diag) for od in diags):
                    # Not one fewer of this diagnostic (nested braces legitimately report the next
                    # level, but there is then one fewer).
                    what = "not-fixed"
                if what:
                    found.append({"what": what, "diagnostic": diag["message"], "code": key(diag)[0],
                                  "level": diag.get("level"), "parts": parts,
                                  "new_errors": sorted(c or m for c, m in errors(fdiags) - base_errors)[:5],
                                  "fixed_name": f"fix{tried}.rs"})
                    shutil.copy(fsrc, d / f"keep-fix{tried}.rs")
        record["tried"] = tried
        record["found"] = [f"{f['what']}: {f['code'] or f['level']} {f['diagnostic'][:80]}" for f in found]
        if found:
            out = WORK / "findings" / rel.replace("/", "__")
            shutil.rmtree(out, ignore_errors=True)
            out.mkdir(parents=True)
            shutil.copy(path, out / path.name)
            for f in found:
                shutil.copy(d / f"keep-{f['fixed_name']}", out / f["fixed_name"])
            (out / "finding.json").write_text(json.dumps({"test": rel, "flags": flags, "edition": edition,
                                                          "found": found}, indent=1))
        return record, found


def main():
    wanted = None
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    skip = None if args.with_rustfix else (lambda text, flags: re.search(r"^//@\s*run-rustfix", text, re.M))
    todo = []
    for path, flags, edition, kind in uitest.tests(args.tests, KINDS, skip):
        rel = str(path.relative_to(args.tests))
        if rel in known or (args.only and args.only not in rel) or (wanted is not None and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    print(f"{len(todo)} tests", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()

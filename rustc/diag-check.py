#!/usr/bin/env python3
"""Diagnostic invariants: what every diagnostic rustc prints must satisfy, whatever the program.

Compiles each standalone UI test with `--error-format=json` and checks every diagnostic
(children and suggestions included):

  internal   user-facing text (message, labels, suggested code) contains compiler-internal
             debug output: `DefId(`, region and type-variable debug names (`ReLateParam`,
             `ReBound`, `ReVar`, `'{erased}`, `?0t`, `^0`), `Opaque(DefId`, `{closure#0}` in
             suggested code
  span       a span outside its file: byte offsets past the end, lines past the last line,
             a start after its end
  nowhere    an error without any span, other than summaries ("aborting due to")
  duplicate  the same diagnostic (level, code, message, primary span) twice (noted, not a
             finding: some are blessed in .stderr files)

    rustc/diag-check.py --rustc <rustc> --tests <rust>/tests/ui --work <dir> [--only <substr>]
        [--known <file>] [--jobs 8] [--pause-on-finding] [--recheck]
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import uitest  # noqa: E402

KINDS = ("check-pass", "build-pass", "run-pass", "check-fail", "build-fail", "run-fail", None)
INTERNAL = re.compile(r"DefId\(|\bRe(LateParam|Bound|Var|Early|Static)\b|'\{erased\}|\?\d+[tif]\b|"
                      r"'\^\d+(_\d+)?\b|Opaque\(DefId|\bAlias\((Projection|Opaque|Inherent|Free)|"
                      r"\bBoundRegionKind|\bDefPath\b|\bLocalDefId\b|\bTyKind::")
# In suggested code, compiler-made names that are not Rust.
INTERNAL_CODE = re.compile(r"\{closure#\d+\}|\{opaque#\d+\}|\{async block@|\{impl#\d+\}|\{constant#\d+\}")

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


def walk(diag):
    yield diag
    for c in diag.get("children", []):
        yield from walk(c)


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    record = {"test": rel}
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        argv = [args.rustc, str(path.resolve()), "--edition", edition or "2015", "--emit=metadata", "-o",
                str(d / "x"), "-Zunstable-options", "-Ainternal_features", "-Aincomplete_features",
                "--error-format=json", *flags]
        try:
            r = subprocess.run(argv, capture_output=True, text=True, timeout=120, cwd=d,
                               env=dict(os.environ, RUSTC_BOOTSTRAP="1", RUST_BACKTRACE="0"))
        except subprocess.TimeoutExpired:
            record["skip"] = "timeout"
            return record, []
    if uitest.is_ice(r.stderr):
        record["skip"] = "ice"
        return record, []
    diags = []
    for line in r.stderr.splitlines():
        try:
            diags.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    files = {}

    def file_info(name):
        if name not in files:
            try:
                data = Path(name).read_bytes() if Path(name).is_absolute() else (path.parent / name).read_bytes()
                files[name] = (len(data), data.count(b"\n") + 1)
            except OSError:
                files[name] = None
        return files[name]

    found, notes = [], []
    seen = Counter()
    for diag in diags:
        prim = next(((s["file_name"], s["byte_start"], s["byte_end"]) for s in diag.get("spans", [])
                     if s.get("is_primary")), None)
        seen[(diag.get("level"), (diag.get("code") or {}).get("code"), diag["message"], prim)] += 1
        if (diag.get("level") == "error" and not diag.get("spans") and not diag.get("children")
                and not re.match(r"aborting due to|could not compile|\d+ (previous )?errors?", diag["message"])
                and "#![feature" not in diag["message"]):
            notes.append({"what": "nowhere", "message": diag["message"][:200]})
        for node in walk(diag):
            texts = [node["message"]] + [s.get("label") or "" for s in node.get("spans", [])]
            for t in texts:
                m = INTERNAL.search(t)
                if m:
                    found.append({"what": "internal", "token": m.group(0), "text": t[:300],
                                  "code": (diag.get("code") or {}).get("code")})
            parts = []
            for s in node.get("spans", []):
                repl = s.get("suggested_replacement")
                if repl is not None:
                    m = INTERNAL.search(repl) or INTERNAL_CODE.search(repl)
                    if m:
                        found.append({"what": "internal", "token": m.group(0), "text": f"suggests {repl[:200]!r}",
                                      "code": (diag.get("code") or {}).get("code")})
                    parts.append((s["file_name"], s["byte_start"], s["byte_end"]))
                info = file_info(s["file_name"]) if not s["file_name"].startswith("<") else None
                if info:
                    size, lines = info
                    if (s["byte_start"] > s["byte_end"] or s["byte_end"] > size or s["line_start"] > lines
                            or s["line_end"] > lines or s["line_start"] > s["line_end"]):
                        found.append({"what": "span", "span": {k: s[k] for k in ("file_name", "byte_start",
                                      "byte_end", "line_start", "line_end")}, "size": size, "lines": lines,
                                      "message": node["message"][:200]})

    for k, n in seen.items():
        if n > 1 and k[0] in ("error", "warning"):
            notes.append({"what": "duplicate", "message": k[2][:200], "times": n})
    # One entry per distinct problem.
    uniq = {json.dumps(f, sort_keys=True): f for f in found}
    found = list(uniq.values())
    record["found"] = [f"{f['what']}: {f.get('token') or ''} {(f.get('text') or f.get('message') or '')[:100]}" for f in found]
    record["notes"] = [f"{n['what']}: {n['message'][:100]}" for n in notes]
    if found:
        out = WORK / "findings" / rel.replace("/", "__")
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        shutil.copy(path, out / path.name)
        (out / "finding.json").write_text(json.dumps({"test": rel, "flags": flags, "edition": edition,
                                                      "found": found, "notes": notes}, indent=1))
    return record, found


def main():
    wanted = None
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    # Tests that ask for compiler internals on purpose: verbose printing, dump attributes.
    debug = lambda text, flags: (any(re.search(r"verbose|-Zdump|unpretty|print-", f) for f in flags)
                                 or re.search(r"#!?\[rustc_(dump|effective_visibility|regions|variance|"
                                              r"outlives|layout|abi|def_path|symbol_name|object_lifetime_default|"
                                              r"dump_[a-z_]+|evaluate_where_clauses|then_this_would_need|"
                                              r"if_this_changed|clean|partition)", text)
                                 or "assumptions_on_binders" in text)
    todo = []
    for path, flags, edition, kind in uitest.tests(args.tests, KINDS, debug):
        rel = str(path.relative_to(args.tests))
        if rel in known or (args.only and args.only not in rel) or (wanted is not None and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    print(f"{len(todo)} tests", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()

#!/usr/bin/env python3
"""Walk option transitions on a fixture: for each row of a PICT table made from a
`flag-model.py --transitions` model, build the fixture clean with the A_ options, rebuild it
incrementally with the B_ options, build it clean with the B_ options, and compare the
rebuild with the clean build (metadata, object code, binary, diagnostics, the binary's
output), as fuzz.py does after an edit.

    rustc/flag-walk.py --rustc <rustc> --fixture fixtures/sink --flags <flag-universe work>
        --table rows.tsv --work <dir> [--workers 8] [--rows a:b]

Rows change options only; the source is not edited.

The options go in RUSTFLAGS with `--target` set, so they apply to the fixture's crates but
not to its build scripts and proc macros. RUSTC_VERIFY_REUSE and RUSTC_REPORT_UNTRACKED are
set, for a compiler with mirth's local patches.

Writes <work>/results.jsonl (one line per row) and <work>/findings/<row>/ for each row whose
rebuild differs from the clean build, or which crashed.

To stay at the frontier: with --pause-on-finding the walk stops taking rows at the first
finding not marked known and writes <work>/PAUSED. Patch the compiler, then run the same
command with --rustc <patched> --recheck: the rows with findings run again first, then the
rows not yet walked. A rerun never repeats rows that are done.
"""

import argparse
import csv
import difflib
import importlib.util
import json
import os
import random
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import artifacts  # noqa: E402
import mutations  # noqa: E402

spec = importlib.util.spec_from_file_location("flag_model", Path(__file__).with_name("flag-model.py"))
flag_model = importlib.util.module_from_spec(spec)
spec.loader.exec_module(flag_model)

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--fixture", required=True)
p.add_argument("--flags", required=True, help="flag-universe.py's work directory")
p.add_argument("--table", required=True)
p.add_argument("--work", required=True)
p.add_argument("--workers", type=int, default=8)
p.add_argument("--toolchain", default="nightly-2026-10-06")
p.add_argument("--target", default="x86_64-unknown-linux-gnu")
p.add_argument("--timeout", type=int, default=600)
p.add_argument("--rows", default="", help="a:b, a slice of the table")
p.add_argument("--edits", type=int, default=0, help="random source edits between A and B (fuzz.py's)")
p.add_argument("--seed", type=int, default=0)
p.add_argument("--pause-on-finding", action="store_true",
               help="stop taking rows at the first finding not marked known; rerun to resume")
p.add_argument("--recheck", action="store_true",
               help="on resume, run the rows that had findings again first (after patching rustc)")
p.add_argument("--p5-builds", type=int, default=12, help="clean rebuilds before a difference counts as reuse")
args = p.parse_args()

FIXTURE = Path(args.fixture).resolve()
WORK = Path(args.work).resolve()
opts = {flag_model.pname(o["flag"] + o["name"]): o for o in json.load(open(Path(args.flags) / "options.json"))}


def flags(row, side):
    """RUSTFLAGS for one side of a row. Cargo passes `-Cembed-bitcode=no` unless its profile
    asks for LTO, and the profile's LTO reaches only the final artifacts, so `-Clto` goes in
    RUSTFLAGS with `-Cembed-bitcode=yes` after Cargo's flag."""
    out = [flag_model.FLAG_BASE]
    for k, v in row.items():
        if k.startswith(side + "_") and v != "absent":
            o = opts[k[2:]]
            f = o["flag"] + o["name"]
            out.append(f if v == "present" else f + "=" + v.replace(";", ","))
    if row.get(side + "_Clto") in ("yes", "on", "thin", "fat") and row.get(side + "_Cembed_bitcode") == "absent":
        out.append("-Cembed-bitcode=yes")
    return out


def build(src, target, rustflags):
    e = dict(os.environ)
    e.update(RUSTC=args.rustc, RUSTC_WRAPPER="", CARGO_INCREMENTAL="1", CARGO_TERM_COLOR="never",
             RUSTFLAGS=" ".join(rustflags), RUSTC_VERIFY_REUSE="1", RUSTC_REPORT_UNTRACKED="1")
    try:
        r = subprocess.run(["cargo", f"+{args.toolchain}", "build", "--workspace", "--offline", "-j", "4",
                            "--target", args.target, "--target-dir", str(target),
                            "--message-format=json-render-diagnostics"],
                           cwd=src, env=e, capture_output=True, text=True, timeout=args.timeout)
        rc, out, log = r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired as t:
        rc, out, log = -1, t.stdout or "", (t.stderr or "") + f"\nkilled after {args.timeout}s"
        out = out.decode() if isinstance(out, bytes) else out
        log = log.decode() if isinstance(log, bytes) else log
    exe = None
    for line in out.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") == "compiler-artifact" and msg.get("executable") and msg["target"]["name"] == FIXTURE.name:
            exe = msg["executable"]
    return {"ok": rc == 0, "log": log,
            "ice": "internal compiler error" in log or "the compiler unexpectedly panicked" in log
                   or "rustc interrupted by SIG" in log,
            "reuse": sorted({l.split(":", 1)[1].strip()[:200] for l in log.splitlines()
                             if l.startswith("rustc-verify-reuse:")}),
            "untracked": sorted({l.strip() for l in log.splitlines() if l.startswith("rustc-untracked-read:")}),
            "error": next((l for l in log.splitlines() if l.startswith("error")), ""),
            "errors": [l[:300] for l in log.splitlines() if l.startswith(("error", "rustc-LLVM ERROR", "LLVM ERROR"))
                       and "could not compile" not in l][:6],
            "art": artifacts.collect(out, target) if rc == 0 else None, "exe": exe}


def run_exe(exe):
    if not exe:
        return None
    try:
        r = subprocess.run([exe], capture_output=True, text=True, timeout=30)
        return [r.returncode, r.stdout[-2000:]]
    except subprocess.TimeoutExpired:
        return ["timeout", ""]


def edit(src, rng, n):
    """Apply n random edits to the fixture's sources; returns the unified diff."""
    diff = ""
    for k in range(n):
        paths = sorted(p for p in src.rglob("*.rs") if "target" not in p.parts)
        for _ in range(20):
            path = rng.choice(paths)
            fn = rng.choices([e for e, _ in mutations.EDITS], weights=[w for _, w in mutations.EDITS])[0]
            if fn in (mutations.str_literal, mutations.int_literal) and path.name == "build.rs":
                continue  # as in fuzz.py: stale OUT_DIR files, or a build script that loops
            old = path.read_text()
            new = fn(old, rng, k)
            if new is not None and new != old:
                path.write_text(new)
                rel = str(path.relative_to(src))
                diff += "".join(difflib.unified_diff(old.splitlines(True), new.splitlines(True),
                                                     "a/" + rel, "b/" + rel))
                break
    return diff


def walk(i_row):
    i, row = i_row
    if (WORK / "PAUSED").exists() or (WORK / "STOP").exists():
        return None
    home = WORK / f"r{i}"
    if home.exists():
        shutil.rmtree(home)
    src, target, inc_target = home / "src", home / "target", home / "target-inc"
    shutil.copytree(FIXTURE, src, ignore=shutil.ignore_patterns("target", "edits", "edit"))
    a, b = flags(row, "A"), flags(row, "B")
    res = {"row": i, "A": a[1:], "B": b[1:]}
    first = build(src, target, a)
    res["A_ok"] = first["ok"]
    findings = []
    if first["ice"]:
        findings.append("ICE in clean A")
    if first["ok"]:
        res["diff"] = edit(src, random.Random(args.seed * 1_000_003 + i), args.edits)
        inc = build(src, target, b)
        target.rename(inc_target)
        clean = build(src, target, b)
        res.update(inc_ok=inc["ok"], clean_ok=clean["ok"], reuse=inc["reuse"], untracked=inc["untracked"])
        if inc["ice"]:
            findings.append("ICE in rebuild B")
        if clean["ice"]:
            findings.append("ICE in clean B")
        if inc["ok"] != clean["ok"]:
            findings.append("split: rebuild " + ("ok" if inc["ok"] else "failed") +
                            ", clean " + ("ok" if clean["ok"] else "failed"))
        if inc["ok"] and clean["ok"]:
            diff = artifacts.compare(inc["art"], clean["art"])
            if row.get("B_Csplit_debuginfo") in ("packed", "unpacked"):
                # Objects and binary name .dwo files by session (DW_AT_GNU_dwo_name, and the
                # dwo_id hashed from it), so two clean builds differ too.
                diff.pop("rlib", None)
                diff.pop("exe", None)
            ra = run_exe(inc["exe"].replace(str(target), str(inc_target)) if inc["exe"] else None)
            rb = run_exe(clean["exe"])
            if ra != rb:
                findings.append(f"run: {ra} vs {rb}")
            if diff:
                # Clean builds may differ among themselves (P5), sometimes only one time in
                # five: build clean again up to --p5-builds times before calling it reuse.
                p5 = {}
                for _ in range(args.p5_builds):
                    shutil.rmtree(target)
                    again = build(src, target, b)
                    if again["ok"]:
                        p5 = artifacts.compare(clean["art"], again["art"])
                    if p5 or not again["ok"]:
                        break
                kind = "P5 " if p5 else ""
                findings += [f"{kind}{k}: {v[:5]}" for k, v in diff.items()]
        res["error"] = inc["error"] or clean["error"]
        res["errors"] = inc["errors"] or clean["errors"]
        if findings:
            d = WORK / "findings" / f"r{i}"
            d.mkdir(parents=True, exist_ok=True)
            (d / "row.json").write_text(json.dumps({**res, "findings": findings}, indent=1))
            if res.get("diff"):
                (d / "edit.diff").write_text(res["diff"])
            (d / "inc.log").write_text(inc["log"][-20000:])
            (d / "clean.log").write_text(clean["log"][-20000:])
    else:
        res["error"] = first["error"]
        res["errors"] = first["errors"]
    res["findings"] = findings
    res["rustc"] = args.rustc
    new = [f for f in findings if not f.startswith("known")]
    if new and args.pause_on_finding:
        (WORK / "PAUSED").write_text(json.dumps({"row": i, "findings": new}, indent=1))
    shutil.rmtree(home)
    with open(WORK / "results.jsonl", "a") as f:
        f.write(json.dumps(res) + "\n")
    print(f"row {i}: A {'ok' if res['A_ok'] else 'failed'}"
          + (f", rebuild {'ok' if res.get('inc_ok') else 'failed'}" if res["A_ok"] else "")
          + (f"; {findings}" if findings else "") + (f"; {res['error'][:100]}" if res.get("error") else ""),
          flush=True)
    return res


def latest():
    """The last result of each row, from <work>/results.jsonl."""
    out = {}
    path = WORK / "results.jsonl"
    if path.exists():
        for line in path.read_text().splitlines():
            r = json.loads(line)
            out[r["row"]] = r
    return out


if __name__ == "__main__":
    WORK.mkdir(parents=True, exist_ok=True)
    (WORK / "PAUSED").unlink(missing_ok=True)
    rows = list(csv.DictReader(open(args.table), delimiter="\t"))
    idx = list(range(len(rows)))
    if args.rows:
        lo, hi = (int(x) if x else None for x in args.rows.split(":"))
        idx = idx[lo:hi]
    # Resume: rows with a result are done, except, with --recheck, those with findings, which
    # run first.
    done = latest()
    again = [i for i in idx if i in done and done[i]["findings"]] if args.recheck else []
    idx = again + [i for i in idx if i not in done]
    if again:
        print(f"rechecking rows {again}", flush=True)
    with ThreadPoolExecutor(args.workers) as ex:
        list(ex.map(walk, [(i, rows[i]) for i in idx]))
    results = list(latest().values())
    summary = {"rows": len(rows), "done": len(results), "A ok": sum(r["A_ok"] for r in results),
               "compared": sum(bool(r.get("inc_ok") and r.get("clean_ok")) for r in results),
               "findings": sum(bool(r["findings"]) for r in results)}
    if (WORK / "PAUSED").exists():
        summary["paused"] = json.loads((WORK / "PAUSED").read_text())
    print(json.dumps(summary))

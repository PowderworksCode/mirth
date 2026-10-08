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
"""

import argparse
import csv
import importlib.util
import json
import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import artifacts  # noqa: E402

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
            "ice": "internal compiler error" in log or "the compiler unexpectedly panicked" in log,
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


def walk(i_row):
    i, row = i_row
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
            if diff:
                findings += [f"{k}: {v[:5]}" for k, v in diff.items()]
            ra = run_exe(inc["exe"].replace(str(target), str(inc_target)) if inc["exe"] else None)
            rb = run_exe(clean["exe"])
            if ra != rb:
                findings.append(f"run: {ra} vs {rb}")
        res["error"] = inc["error"] or clean["error"]
        res["errors"] = inc["errors"] or clean["errors"]
        if findings:
            d = WORK / "findings" / f"r{i}"
            d.mkdir(parents=True, exist_ok=True)
            (d / "row.json").write_text(json.dumps({**res, "findings": findings}, indent=1))
            (d / "inc.log").write_text(inc["log"][-20000:])
            (d / "clean.log").write_text(clean["log"][-20000:])
    else:
        res["error"] = first["error"]
        res["errors"] = first["errors"]
    res["findings"] = findings
    shutil.rmtree(home)
    with open(WORK / "results.jsonl", "a") as f:
        f.write(json.dumps(res) + "\n")
    print(f"row {i}: A {'ok' if res['A_ok'] else 'failed'}"
          + (f", rebuild {'ok' if res.get('inc_ok') else 'failed'}" if res["A_ok"] else "")
          + (f"; {findings}" if findings else "") + (f"; {res['error'][:100]}" if res.get("error") else ""),
          flush=True)
    return res


if __name__ == "__main__":
    WORK.mkdir(parents=True, exist_ok=True)
    rows = list(csv.DictReader(open(args.table), delimiter="\t"))
    idx = list(range(len(rows)))
    if args.rows:
        lo, hi = (int(x) if x else None for x in args.rows.split(":"))
        idx = idx[lo:hi]
    with ThreadPoolExecutor(args.workers) as ex:
        results = list(ex.map(walk, [(i, rows[i]) for i in idx]))
    print(json.dumps({"rows": len(results), "A ok": sum(r["A_ok"] for r in results),
                      "compared": sum(bool(r.get("inc_ok") and r.get("clean_ok")) for r in results),
                      "findings": sum(bool(r["findings"]) for r in results)}))

#!/usr/bin/env python3
"""Coverage of the compiler across option configurations: build a fixture with a
coverage-instrumented rustc (rustc/coverage.toml) once per row of a PICT transitions table,
clean with the A options, then rebuilt after one random edit with the B options, each row's
rustc processes logging to <out>/row<i>/. Read the result with rustc/coverage.py.

    rustc/coverage-flags.py --rustc <instrumented rustc> --fixture fixtures/sink
        --flags <flag-universe work> --table rows.tsv --out <dir> [--rows 0:40] [--workers 6]
"""

import argparse
import csv
import importlib.util
import json
import os
import random
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

here = Path(__file__).parent
sys.path.insert(0, str(here))
import mutations  # noqa: E402

spec = importlib.util.spec_from_file_location("flag_model", here / "flag-model.py")
flag_model = importlib.util.module_from_spec(spec)
spec.loader.exec_module(flag_model)

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--fixture", required=True)
p.add_argument("--flags", required=True)
p.add_argument("--table", required=True)
p.add_argument("--out", required=True)
p.add_argument("--rows", default="")
p.add_argument("--workers", type=int, default=6)
p.add_argument("--toolchain", default="nightly-2026-10-06")
args = p.parse_args()

OUT = Path(args.out).resolve()
opts = {flag_model.pname(o["flag"] + o["name"]): o for o in json.load(open(Path(args.flags) / "options.json"))}
rows = list(csv.DictReader(open(args.table), delimiter="\t"))
idx = list(range(len(rows)))
if args.rows:
    lo, hi = (int(x) if x else None for x in args.rows.split(":"))
    idx = idx[lo:hi]


def flags(row, side):
    out = [flag_model.FLAG_BASE]
    for k, v in row.items():
        if k.startswith(side + "_") and v != "absent":
            o = opts[k[2:]]
            f = o["flag"] + o["name"]
            out.append(f if v == "present" else f + "=" + v.replace(";", ","))
    return out


def run(i):
    logs = OUT / f"row{i}"
    if logs.exists():
        return i, "done before"
    work = OUT / f"work{i}"
    shutil.rmtree(work, ignore_errors=True)
    src = work / "s"
    shutil.copytree(args.fixture, src, ignore=shutil.ignore_patterns("target"))
    results = []
    for side in "AB":
        if side == "B":
            rng = random.Random(i)
            paths = sorted(p for p in src.rglob("*.rs") if "target" not in p.parts)
            for _ in range(20):
                path = rng.choice(paths)
                fn = rng.choices([e for e, _ in mutations.EDITS], weights=[w for _, w in mutations.EDITS])[0]
                if path.name == "build.rs":
                    continue
                new = fn(path.read_text(), rng, 0)
                if new is not None:
                    path.write_text(new)
                    break
        e = dict(os.environ, RUSTC=args.rustc, RUSTC_WRAPPER="", CARGO_INCREMENTAL="1",
                 RUSTFLAGS=" ".join(flags(rows[i], side)), MIRTH_OUT=str(work / "logs"))
        r = subprocess.run(["cargo", f"+{args.toolchain}", "build", "--workspace", "--offline", "-j", "4",
                            "--target", "x86_64-unknown-linux-gnu", "--target-dir", str(work / "t")],
                           cwd=src, env=e, capture_output=True, text=True, timeout=1800)
        results.append("ok" if r.returncode == 0 else "failed")
    (work / "logs").mkdir(exist_ok=True)
    (work / "logs").rename(logs)
    shutil.rmtree(work, ignore_errors=True)
    return i, " ".join(results)


OUT.mkdir(parents=True, exist_ok=True)
with ThreadPoolExecutor(args.workers) as ex:
    for i, result in ex.map(run, idx):
        print(f"row {i}: {result}", flush=True)

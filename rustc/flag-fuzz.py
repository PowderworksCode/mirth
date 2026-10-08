#!/usr/bin/env python3
"""Run fuzz.py under option configurations: for each chosen row of a PICT table (the B side of
a `flag-model.py --transitions --cargo` model), fuzz the fixture with those options in
RUSTFLAGS for a number of edits. Stops at the first finding (fuzz.py --pause-on-finding);
rerunning resumes after the rows already done.

    rustc/flag-fuzz.py --rustc <rustc> --fixture fixtures/sink --flags <flag-universe work>
        --table rows.tsv --work <dir> [--rows 0:20] [--edits 200] [--workers 8]

Writes <dir>/row<i>/ (fuzz.py's work directory) and <dir>/rows.jsonl (one line per finished
row: options, fuzz.py's totals, findings).
"""

import argparse
import csv
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("flag_model", Path(__file__).with_name("flag-model.py"))
flag_model = importlib.util.module_from_spec(spec)
spec.loader.exec_module(flag_model)

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--fixture", required=True)
p.add_argument("--flags", required=True)
p.add_argument("--table", required=True)
p.add_argument("--work", required=True)
p.add_argument("--rows", default="")
p.add_argument("--edits", type=int, default=200, help="per row, over all workers")
p.add_argument("--workers", type=int, default=8)
args = p.parse_args()

WORK = Path(args.work).resolve()
WORK.mkdir(parents=True, exist_ok=True)
opts = {flag_model.pname(o["flag"] + o["name"]): o for o in json.load(open(Path(args.flags) / "options.json"))}
rows = list(csv.DictReader(open(args.table), delimiter="\t"))
idx = list(range(len(rows)))
if args.rows:
    lo, hi = (int(x) if x else None for x in args.rows.split(":"))
    idx = idx[lo:hi]
log = WORK / "rows.jsonl"
done = {json.loads(l)["row"] for l in log.read_text().splitlines()} if log.exists() else set()

for i in idx:
    if i in done:
        continue
    flags = [flag_model.FLAG_BASE]
    for k, v in rows[i].items():
        if k.startswith("B_") and v != "absent":
            o = opts[k[2:]]
            f = o["flag"] + o["name"]
            flags.append(f if v == "present" else f + "=" + v.replace(";", ","))
    w = WORK / f"row{i}"
    r = subprocess.run([sys.executable, str(Path(__file__).with_name("fuzz.py")), "--rustc", args.rustc,
                        "--fixture", args.fixture, "--work", str(w), "--workers", str(args.workers),
                        "--edits", str(max(1, args.edits // args.workers)), "--seed", str(i),
                        "--rustflags", " ".join(flags), "--target", "x86_64-unknown-linux-gnu",
                        "--pause-on-finding"], capture_output=True, text=True)
    total = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""
    paused = (w / "PAUSED").read_text() if (w / "PAUSED").exists() else None
    print(f"row {i}: {total}" + (f" PAUSED {paused}" if paused else ""), flush=True)
    if paused:
        sys.exit(3)  # not recorded as done: rerun after patching to do this row again
    with log.open("a") as f:
        f.write(json.dumps({"row": i, "flags": flags[1:], "total": total}) + "\n")

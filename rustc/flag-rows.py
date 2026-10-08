#!/usr/bin/env python3
"""Compile a trivial crate once per row of a PICT table made from flag-model.py's model, and
count the rows rustc rejects, grouped by first error.

    rustc/flag-rows.py <work> <table.tsv> <rustc> [extra rustc args, e.g. --emit=metadata]

Tables from a --transitions model are not supported.
"""

import csv
import importlib.util
import json
import subprocess
import sys
import tempfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

spec = importlib.util.spec_from_file_location("flag_model", Path(__file__).with_name("flag-model.py"))
flag_model = importlib.util.module_from_spec(spec)
spec.loader.exec_module(flag_model)

work, tsv, rustc = sys.argv[1:4]
extra = sys.argv[4:]
opts = {flag_model.pname(o["flag"] + o["name"]): o for o in json.load(open(work + "/options.json"))}
rows = list(csv.DictReader(open(tsv), delimiter="\t"))


def argv(row):
    out = [flag_model.FLAG_BASE]
    for k, v in row.items():
        if v != "absent":
            f = opts[k]["flag"] + opts[k]["name"]
            out.append(f if v == "present" else f + "=" + v.replace(";", ","))
    return out


def run(row):
    with tempfile.TemporaryDirectory(dir=work) as d:
        Path(d, "lib.rs").write_text("pub fn f(x: u32) -> u32 { x.wrapping_mul(3) }\n")
        a = argv(row)
        p = subprocess.run([rustc, "--edition", "2021", "--crate-type", "lib", *extra, "-o", d + "/out",
                            *a, d + "/lib.rs"], capture_output=True, text=True, cwd=d, timeout=300)
        err = next((l for l in p.stderr.splitlines() if l.startswith("error")), "")
        return p.returncode == 0, err, len(a) - 1


res = list(ThreadPoolExecutor(10).map(run, rows))
bad = [e for ok, e, _ in res if not ok]
print(f"{len(rows)} rows, {len(bad)} rejected, {sum(n for *_, n in res) / len(res):.0f} options per row on average")
for e, c in Counter(e[:110] for e in bad).most_common():
    print(c, e)

#!/usr/bin/env python3
"""Fold coverage logs (MIRTH_OUT, from a compiler built with rustc/coverage.toml) into a running
union as they are finished, and delete them: a test suite run starts tens of thousands of rustc
processes, whose logs together would not fit on the disk.

    rustc/coverage-compact.py --logs <MIRTH_OUT> --out <dir> [--until <file>]

A log is finished when its last line is the `X` line written at exit, or when it has not changed
for ten minutes (a process that crashed). For each, <out>/added.jsonl gets the process's source
file argument and the sites it reached that no earlier process did; <out>/union.txt holds every
site reached so far, rewritten every pass. Runs until <until> exists, then does a last pass.
"""

import argparse
import json
import os
import re
import time
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--logs", required=True)
p.add_argument("--out", required=True)
p.add_argument("--until", default="")
args = p.parse_args()

LOGS, OUT = Path(args.logs), Path(args.out)
OUT.mkdir(parents=True, exist_ok=True)
union_path = OUT / "union.txt"
union = set(union_path.read_text().split()) if union_path.exists() else set()


def source_of(header):
    for field in header.split("\t")[3:]:
        if field.endswith(".rs"):
            return field
    m = re.search(r"--crate-name\t(\S+)", header)
    return m.group(1) if m else ""


def one_pass(final):
    done = 0
    with (OUT / "added.jsonl").open("a") as added:
        for log in list(LOGS.glob("*.log")):
            try:
                text = log.read_text(errors="replace")
                age = time.time() - log.stat().st_mtime
            except FileNotFoundError:
                continue
            lines = text.splitlines()
            if not lines or not (lines[-1].startswith("X\t") or age > 600 or (final and age > 5)):
                continue
            sites = {l[2:] for l in lines if l.startswith("V\t")}
            new = sites - union
            union.update(new)
            if new:
                added.write(json.dumps({"source": source_of(lines[0]), "new": sorted(new)}) + "\n")
            log.unlink()
            done += 1
    union_path.write_text("\n".join(sorted(union)) + "\n")
    return done


while True:
    finishing = bool(args.until) and os.path.exists(args.until)
    n = one_pass(finishing)
    print(f"{time.strftime('%H:%M:%S')} {n} logs folded, {len(union)} sites", flush=True)
    if finishing:
        one_pass(True)
        break
    time.sleep(30)

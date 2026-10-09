#!/usr/bin/env python3
"""Which of the compiler's functions ran, from a compiler built with rustc/coverage.toml.

    rustc/coverage.py --sites <build>/mirth-sites --logs <MIRTH_OUT dir> [--logs ...]
        [--json out.json] [--files] [--unhit <crate or file substring>]

The site tables list each instrumented function (`cover` sites: id, crate, path, span); every
rustc process run with MIRTH_OUT set writes, at exit, a `V <site>` line for each function it
entered. This reads both and prints, per crate, how many functions ran; with --files, per
source file; with --unhit, the functions that never ran, in the crates or files matching.

Several --logs directories (several runs: fixtures, flags) are combined.
"""

import argparse
import json
from collections import defaultdict
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--sites", required=True)
p.add_argument("--logs", action="append", required=True)
p.add_argument("--json")
p.add_argument("--files", action="store_true")
p.add_argument("--unhit", action="append", default=[])
args = p.parse_args()

functions = {}  # site -> (crate, path, span)
for table in Path(args.sites).glob("*.sites"):
    for line in table.read_text(errors="replace").splitlines():
        f = line.split("\t")
        if len(f) >= 7 and f[1] == "cover":
            functions[f[0]] = (f[3], f[4], f[6])

hit = set()
processes = 0
for directory in args.logs:
    for log in Path(directory).glob("*.log"):
        processes += 1
        for line in log.read_text(errors="replace").splitlines():
            if line.startswith("V\t"):
                hit.add(line[2:])

unknown = hit - functions.keys()
by_crate = defaultdict(lambda: [0, 0])
by_file = defaultdict(lambda: [0, 0])
for site, (krate, path, span) in functions.items():
    file = span.rsplit(":", 2)[0]
    ran = site in hit
    for table, key in ((by_crate, krate), (by_file, file)):
        table[key][0] += 1
        table[key][1] += ran

total, ran = len(functions), len(hit & functions.keys())
print(f"{processes} processes; {ran} of {total} functions ran ({100 * ran / max(total, 1):.1f}%)"
      + (f"; {len(unknown)} sites not in the tables" if unknown else ""))
print(f"{'crate':40} {'ran':>7} {'of':>7} {'%':>6}")
for krate, (n, r) in sorted(by_crate.items(), key=lambda kv: kv[1][1] / kv[1][0]):
    print(f"{krate:40} {r:7} {n:7} {100 * r / n:6.1f}")
if args.files:
    print()
    print(f"{'file':80} {'ran':>6} {'of':>6}")
    for file, (n, r) in sorted(by_file.items(), key=lambda kv: (kv[1][1] / kv[1][0], -kv[1][0])):
        print(f"{file:80} {r:6} {n:6}")
for pattern in args.unhit:
    print(f"\nnever ran, matching {pattern!r}:")
    for site, (krate, path, span) in sorted(functions.items(), key=lambda kv: kv[1][2]):
        if site not in hit and (pattern in krate or pattern in span):
            print(f"  {path}  {span}")
if args.json:
    Path(args.json).write_text(json.dumps({
        "processes": processes, "functions": total, "ran": ran,
        "crates": {k: {"functions": n, "ran": r} for k, (n, r) in by_crate.items()},
        "files": {k: {"functions": n, "ran": r} for k, (n, r) in by_file.items()},
        "unhit": sorted(f"{functions[s][1]}\t{functions[s][2]}" for s in functions.keys() - hit)}, indent=1))

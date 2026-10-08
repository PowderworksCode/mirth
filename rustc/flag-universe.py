#!/usr/bin/env python3
"""Enumerate rustc's -C and -Z options, find which values and pairs of values it accepts,
and size covering arrays over them.

    rustc/flag-universe.py --rustc <rustc> --source <rust checkout> --work <dir> [--jobs N]

Each option's domain is its absence plus the values worth trying: `yes`/`no` for a boolean,
present for an option without a value, the values its parser's description lists for an
enumerated one, two samples for a number. Options taking free-form strings, paths or lists
are counted but left out. Every value is tried alone on a trivial crate (`--emit=metadata`,
so only option checking and a tiny compilation run), then every pair of accepted values of
different options. A pair is "rejected" when rustc fails with the pair but accepts each value
alone.

Writes <work>/options.json (domains), <work>/singles.json, <work>/pairs.json, and prints
the sizes of pairwise covering arrays built greedily over the accepted domains.
"""

import argparse
import itertools
import json
import os
import random
import re
import subprocess
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--source", required=True, help="a rust checkout, for compiler/rustc_session/src/options.rs")
p.add_argument("--work", required=True)
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--skip-pairs", action="store_true")
args = p.parse_args()

work = Path(args.work)
work.mkdir(parents=True, exist_ok=True)
src = (Path(args.source) / "compiler/rustc_session/src/options.rs").read_text()

# The parsers' descriptions, joined across lines.
descs = {}
for m in re.finditer(r'pub\(crate\) const (parse_\w+): &str =\s*((?:"(?:[^"\\]|\\.)*"\s*)+|parse_\w+|[^;]+);', src):
    descs[m.group(1)] = m.group(2)
for k, v in list(descs.items()):
    if re.fullmatch(r"parse_\w+", v.strip()):
        descs[k] = descs.get(v.strip(), "")

BOOL = {"parse_bool", "parse_opt_bool"}
NO_VALUE = {"parse_no_value"}
NUMBER = {"parse_number", "parse_opt_number"}
FREE = {"parse_string", "parse_opt_string", "parse_string_push", "parse_opt_pathbuf", "parse_list",
        "parse_comma_list", "parse_opt_comma_list", "parse_ignore", "parse_target_feature",
        "parse_list_with_polarity", "parse_llvm_module_flag", "parse_patchable_function_entry",
        "parse_autodiff", "parse_offload", "parse_allow_partial_mitigations",
        "parse_deny_partial_mitigations", "parse_rust_version", "parse_unpretty",
        "parse_passes", "parse_branch_protection", "parse_instrument_xray",
        "parse_linker_features", "parse_link_self_contained", "parse_align",
        "parse_location_detail", "parse_coverage_options", "parse_codegen_retag_options"}


def enum_values(parser):
    vals = re.findall(r"`([^`]+)`", descs.get(parser, ""))
    out = []
    for v in vals:
        if v in out or " " in v or "<" in v or "=" in v:
            continue
        out.append(v)
    return out


options = []
for flag, grp in (("-C", "CodegenOptions"), ("-Z", "UnstableOptions")):
    i = src.index("options! {\n    " + grp)
    j = src.index("\n}", i)
    for line in src[i:j].splitlines():
        m = re.match(r"^\s*(\w+): .*?, (parse_\w+), \[(\w+)", line)
        if not m:
            continue
        name, parser, tracking = m.group(1).replace("_", "-"), m.group(2), m.group(3)
        if parser in BOOL:
            values = ["yes", "no"]
        elif parser in NO_VALUE:
            values = [None]
        elif parser in NUMBER:
            values = ["1", "16"]
        elif parser in FREE:
            values = []
        else:
            values = enum_values(parser)
        options.append({"flag": flag, "name": name, "parser": parser, "tracking": tracking,
                        "values": values, "free": parser in FREE or not values})


def arg(o, v):
    return f"{o['flag']}{o['name']}" if v is None else f"{o['flag']}{o['name']}={v}"


def run(argv):
    with tempfile.TemporaryDirectory(dir=work) as d:
        lib = Path(d) / "lib.rs"
        lib.write_text("pub fn f(x: u32) -> u32 { x.wrapping_mul(3) }\n")
        try:
            r = subprocess.run([args.rustc, "--edition", "2021", "--crate-type", "lib", "--emit=metadata",
                                "-o", str(Path(d) / "out.rmeta"), *argv, str(lib)],
                               capture_output=True, text=True, timeout=60, cwd=d)
            err = r.stderr
            first = next((l for l in err.splitlines() if l.startswith(("error", "warning"))), "")
            return {"ok": r.returncode == 0, "warn": "warning" in err, "msg": first[:200]}
        except subprocess.TimeoutExpired:
            return {"ok": False, "warn": False, "msg": "timeout"}


(work / "options.json").write_text(json.dumps(options, indent=1))
walkable = [o for o in options if not o["free"]]
print(f"{len(options)} options: {len(walkable)} with enumerable values, "
      f"{len(options) - len(walkable)} free-form")

singles_path = work / "singles.json"
if singles_path.exists():
    singles = json.loads(singles_path.read_text())
else:
    jobs = [(o, v) for o in walkable for v in o["values"]]
    with ThreadPoolExecutor(args.jobs) as ex:
        results = list(ex.map(lambda ov: run([arg(*ov)]), jobs))
    singles = [{"arg": arg(o, v), "option": o["flag"] + o["name"], "value": v, **r}
               for (o, v), r in zip(jobs, results)]
    singles_path.write_text(json.dumps(singles, indent=1))
accepted = {}
for s in singles:
    if s["ok"]:
        accepted.setdefault(s["option"], []).append(s["arg"])
print(f"{len(singles)} single values tried, {sum(map(len, accepted.values()))} accepted "
      f"({len(accepted)} options with at least one)")

rejected_pairs = set()
if not args.skip_pairs:
    pairs_path = work / "pairs.json"
    if pairs_path.exists():
        pairs = json.loads(pairs_path.read_text())
    else:
        opts = sorted(accepted)
        jobs = [(a, b) for x, y in itertools.combinations(opts, 2) for a in accepted[x] for b in accepted[y]]
        # A value rejected alone may need another option: try it with every accepted value
        # of every other option.
        for r in (x for x in singles if not x["ok"]):
            jobs += [(r["arg"], b) for y in opts if y != r["option"] for b in accepted[y]]
        print(f"{len(jobs)} pairs to try", flush=True)
        with ThreadPoolExecutor(args.jobs) as ex:
            results = list(ex.map(lambda ab: run(list(ab)), jobs))
        pairs = [{"a": a, "b": b, **r} for (a, b), r in zip(jobs, results)]
        pairs_path.write_text(json.dumps(pairs, indent=1))
    alone = {x["arg"] for x in singles if x["ok"]}
    rejected_pairs = {(x["a"], x["b"]) for x in pairs if not x["ok"] and x["a"] in alone and x["b"] in alone}
    requires = {}
    for x in pairs:
        if x["ok"] and x["a"] not in alone:
            requires.setdefault(x["a"], []).append(x["b"])
    print(f"{len(pairs)} pairs tried; {len(rejected_pairs)} pairs of values accepted alone "
          f"are rejected together; {len(requires)} values rejected alone are accepted with "
          f"another option")
    (work / "requires.json").write_text(json.dumps(requires, indent=1))


def covering_array(domains, forbidden, seed=0):
    """A pairwise covering array, built greedily: each row is chosen among random
    candidates to cover the most uncovered pairs. Values are indices into each domain; index
    0 is the option's absence."""
    rng = random.Random(seed)
    names = list(domains)
    n = len(names)
    uncovered = set()
    for i, j in itertools.combinations(range(n), 2):
        for a in range(len(domains[names[i]])):
            for b in range(len(domains[names[j]])):
                if (domains[names[i]][a], domains[names[j]][b]) not in forbidden:
                    uncovered.add((i, a, j, b))
    rows = []
    while uncovered:
        best, best_gain = None, -1
        target = next(iter(uncovered))
        for _ in range(30):
            row = [rng.randrange(len(domains[k])) for k in names]
            row[target[0]], row[target[2]] = target[1], target[3]
            # repair forbidden pairs by falling back to absence
            for i, j in itertools.combinations(range(n), 2):
                if (domains[names[i]][row[i]], domains[names[j]][row[j]]) in forbidden:
                    if j not in (target[0], target[2]):
                        row[j] = 0
                    elif i not in (target[0], target[2]):
                        row[i] = 0
            gain = sum(1 for i, j in itertools.combinations(range(n), 2) if (i, row[i], j, row[j]) in uncovered)
            if gain > best_gain:
                best, best_gain = row, gain
        rows.append(best)
        for i, j in itertools.combinations(range(n), 2):
            uncovered.discard((i, best[i], j, best[j]))
    return rows


def summarize(label, opts):
    domains = {o: [None] + accepted[o] for o in opts}
    sizes = sorted((len(v) for v in domains.values()), reverse=True)
    total = 1
    for s in sizes:
        total *= s
    lower = sizes[0] * sizes[1] if len(sizes) > 1 else sizes[0]
    forbidden = {(a, b) for a, b in rejected_pairs} | {(b, a) for a, b in rejected_pairs}
    rows = covering_array(domains, forbidden)
    print(f"{label}: {len(opts)} options, all combinations {total:.3e}, "
          f"pairwise covering array {len(rows)} rows (lower bound {lower})")
    return rows


byopt = {o["flag"] + o["name"]: o for o in options}
results = {}
for label, pred in (("untracked", lambda o: o["tracking"] == "UNTRACKED"),
                    ("tracked", lambda o: o["tracking"] != "UNTRACKED"),
                    ("all", lambda o: True)):
    opts = sorted(k for k in accepted if pred(byopt[k]))
    if len(opts) > 1:
        results[label] = summarize(label, opts)
(work / "covering.json").write_text(json.dumps({k: len(v) for k, v in results.items()}))

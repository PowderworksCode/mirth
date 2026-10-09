#!/usr/bin/env python3
"""Which of the compiler's functions can run at all: reachability over the call graph that a
compiler built with rustc/callgraph.toml writes (`<crate>.graph`), against the functions a
compiler built with rustc/coverage.toml instruments (`cover` sites). The functions that cannot
be reached are taken out of coverage's denominator.

    rustc/callgraph.py --graph <build>/mirth-sites --sites <coverage build>/mirth-sites
        [--hit <union.txt> ...] [--logs <MIRTH_OUT> ...] [--json out.json] [--unreachable <crate>]

The graph over-approximates what can run, so what it leaves out cannot run (as far as the
edges it knows go):

- edges: direct calls, functions and closures used as values, callees MIR inlining merged in,
  and trait calls resolved in the caller's context;
- a call to a trait item reaches every body implementing it, and the trait's own default body;
- a body nested in another (a closure, an inline const) is reached with it, and one nested in
  something that is not a body (a static's or a constant's initializer) is a root;
- roots: the compiler's `main`s, every function with a foreign ABI (callbacks from C, C++ and
  LLVM), every body implementing a trait from outside the compiler,
  which the standard library may call (`Iterator::next`, `Drop::drop`, `Debug::fmt`, ...), and
  every constant's and static's initializer (tables of function pointers, callbacks).

With --hit or --logs, reports coverage of the reachable functions, and checks the analysis: a
function that ran must be reachable; any that are not are listed (an edge kind it misses).
"""

import argparse
import json
import re
from collections import defaultdict, deque
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--graph", required=True)
p.add_argument("--sites", required=True)
p.add_argument("--hit", action="append", default=[])
p.add_argument("--logs", action="append", default=[])
p.add_argument("--json")
p.add_argument("--unreachable", action="append", default=[])
args = p.parse_args()

ROOTS = {"rustc_main::main", "rustc_driver_impl::main"}
# Crates that do not run when the compiler does: proc macros (run while it is built) and the
# Windows resource helper of its build script.
NOT_AT_RUN_TIME = {"rustc_macros", "rustc_type_ir_macros", "rustc_index_macros", "rustc_windows_rc",
                   "rustc_hir_macros", "rustc_fluent_macro"}

# Nodes are DefPathHashes; bodies also have their path (as coverage sites name them).
path_of, hash_of = {}, {}
implements, external_impl, const_bodies = {}, set(), set()
edges = defaultdict(set)
for f in Path(args.graph).glob("*.graph"):
    for line in f.read_text(errors="replace").splitlines():
        parts = line.split("\t")
        if parts[0] == "body":
            node, path, item, item_path, kind = parts[1], parts[2], parts[3], parts[4], parts[5]
            path_of[node] = path
            hash_of[path] = node
            if kind in ("const", "extern"):
                const_bodies.add(node)
            if item != "-":
                implements[node] = item
                if not item_path.startswith("rustc_"):
                    external_impl.add(node)
        elif parts[0] == "edge":
            edges[parts[1]].add(parts[2])
bodies = set(path_of)

implementors = defaultdict(set)
for body, item in implements.items():
    implementors[item].add(body)


def parent(path):
    m = re.match(r"^(.*)::\{[^}]*\}$", path)
    return m.group(1) if m else None


children = defaultdict(set)
roots = {hash_of[r] for r in ROOTS if r in hash_of} | external_impl | const_bodies
for node, path in path_of.items():
    up = parent(path)
    if up is None:
        continue
    # Nested in a body, or in a static's or a constant's initializer, which is not one here.
    while up is not None and up not in hash_of and parent(up) is not None:
        up = parent(up)
    if up in hash_of:
        children[hash_of[up]].add(node)
    else:
        roots.add(node)

reachable = set()
queue = deque(roots)
while queue:
    node = queue.popleft()
    if node in reachable:
        continue
    reachable.add(node)
    # A call to a trait item reaches the bodies implementing it.
    for nxt in edges.get(node, set()) | children.get(node, set()) | implementors.get(node, set()):
        if nxt not in reachable:
            queue.append(nxt)
        for impl in implementors.get(nxt, ()):
            if impl not in reachable:
                queue.append(impl)
reachable_paths = {path_of[n] for n in reachable if n in path_of}

functions = {}  # site -> (crate, path)
for table in Path(args.sites).glob("*.sites"):
    for line in table.read_text(errors="replace").splitlines():
        f = line.split("\t")
        if len(f) >= 7 and f[1] == "cover":
            functions[f[0]] = (f[3], f[4])

functions = {s: v for s, v in functions.items() if v[0] not in NOT_AT_RUN_TIME}
paths = {path for _, path in functions.values()}
known = paths & set(hash_of)
unreach = {path for path in known if path not in reachable_paths}
print(f"{len(bodies)} bodies in the graph, {len(roots)} roots, {len(reachable & bodies)} reachable")
print(f"{len(functions)} instrumented functions; {len(known)} in the graph; "
      f"{len(unreach)} unreachable ({100 * len(unreach) / max(len(known), 1):.1f}%)")

hit = set()
for u in args.hit:
    hit |= set(Path(u).read_text().split())
for d in args.logs:
    for log in Path(d).rglob("*.log"):
        for line in log.read_text(errors="replace").splitlines():
            if line.startswith("V\t"):
                hit.add(line[2:])
hit_paths = {functions[s][1] for s in hit if s in functions}
by_crate = defaultdict(lambda: [0, 0, 0])
for site, (krate, path) in functions.items():
    row = by_crate[krate]
    if path in unreach:
        continue
    row[0] += 1
    row[1] += path in hit_paths
if hit:
    total = sum(r[0] for r in by_crate.values())
    ran = sum(r[1] for r in by_crate.values())
    print(f"coverage: {len(hit_paths)} functions ran; of the {total} reachable ones, {ran} "
          f"({100 * ran / max(total, 1):.1f}%)")
    wrong = sorted(hit_paths & unreach)
    print(f"ran although unreachable (edges the analysis misses): {len(wrong)}")
    for w in wrong[:30]:
        print("   ", w)
    print(f"{'crate':40} {'ran':>7} {'reachable':>9} {'%':>6}")
    for krate, (n, r, _) in sorted(by_crate.items(), key=lambda kv: kv[1][1] / max(kv[1][0], 1)):
        if n:
            print(f"{krate:40} {r:7} {n:9} {100 * r / n:6.1f}")
for crate in args.unreachable:
    print(f"\nunreachable in {crate}:")
    for path in sorted(unreach):
        if path.startswith(crate + "::"):
            print("   ", path)
if args.json:
    Path(args.json).write_text(json.dumps({"unreachable": sorted(unreach),
                                           "reachable_not_hit": sorted((paths - unreach) - hit_paths)}, indent=0))

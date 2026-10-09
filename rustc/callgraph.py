#!/usr/bin/env python3
"""Which of the compiler's functions can run at all: reachability over the call graph that a
compiler built with rustc/callgraph.toml writes (`<crate>.graph`), against the functions a
compiler built with rustc/coverage.toml instruments (`cover` sites). The functions that cannot
be reached are taken out of coverage's denominator.

    rustc/callgraph.py --graph <build>/mirth-sites --sites <coverage build>/mirth-sites
        [--hit <union.txt> ...] [--external <union.txt> ...] [--logs <MIRTH_OUT> ...]
        [--json out.json] [--unreachable <crate>] [--why <function>] [--gaps <file>]

The graph over-approximates what can run, so what it leaves out cannot run (as far as the
edges it knows go):

- edges: direct calls, functions and closures used as values, callees MIR inlining merged in,
  and trait calls resolved in the caller's context;
- a call to a trait item reaches every body implementing it, and the trait's own default body;
  but (rapid type analysis) a method of an impl for one of the compiler's structs or enums only
  once reachable code builds that type (an aggregate, a constructor, a constant of it), and a
  function of a trait impl only once reachable code demands the trait for the impl's type (a
  call whose bounds say so, an impl selected for one, a cast to `dyn Trait`): see
  docs/coverage.md for the exceptions;
- a body nested in another (a closure, an inline const) is reached with it, and one nested in
  something that is not a body (a static's or a constant's initializer) is a root;
- roots: the compiler's and rustdoc's `main`s, every function with a foreign ABI (callbacks from C, C++ and
  LLVM), every body implementing a trait from outside the compiler (for one of the compiler's
  types: once the type is built),
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
p.add_argument("--external", action="append", default=[],
               help="union.txt of programs outside the compiler that link it (ui-fulldeps): what they "
                    "ran counts, and is a root, since their own mains call it")
p.add_argument("--json")
p.add_argument("--unreachable", action="append", default=[])
p.add_argument("--why", action="append", default=[], help="print how a function is reached")
p.add_argument("--gaps", help="write the reachable functions that never ran, by crate and file, to this file")
args = p.parse_args()

ROOTS = {"rustc_main::main", "rustc_driver_impl::main", "rustdoc::main"}
# Crates that do not run when the compiler does: proc macros (run while it is built) and the
# Windows resource helper of its build script.
NOT_AT_RUN_TIME = {"rustc_macros", "rustc_type_ir_macros", "rustc_index_macros", "rustc_windows_rc",
                   "rustc_hir_macros", "rustc_fluent_macro"}

# Nodes are DefPathHashes; bodies also have their path (as coverage sites name them).
path_of, hash_of = {}, {}
implements, external_impl, const_bodies = {}, set(), set()
SCOPE = ("rustc_", "rustdoc")
self_type = {}  # a method in an impl for one of the compiler's structs or enums -> that type
constructs = defaultdict(set)  # body -> types it builds
spec_bounds = set()  # traits a specializing impl's bounds name
impl_trait = {}
impl_key = {}  # a function of a trait impl -> (trait, the struct or enum the impl is for)
demands = defaultdict(set)  # body -> (trait, type) pairs it needs implemented
# Called by the language on any value, not through a bound: drop glue.
UNGATED_TRAITS = {"core::ops::drop::Drop"}
edges = defaultdict(set)
for f in Path(args.graph).glob("*.graph"):
    for line in f.read_text(errors="replace").splitlines():
        parts = line.split("\t")
        if parts[0] == "body":
            node, path, item, item_path, kind = parts[1], parts[2], parts[3], parts[4], parts[5]
            if len(parts) > 7 and parts[7].startswith(SCOPE):
                self_type[node] = parts[6]
            # A trait with specializing impls: which impl a call reaches is decided by more than
            # the bounds say, so such impls are not gated on them.
            specialized = len(parts) > 11 and parts[11] == "specialized"
            if (len(parts) > 10 and parts[8] != "-" and parts[10] != "-" and parts[9] not in UNGATED_TRAITS
                    and not specialized):
                impl_key[node] = (parts[8], parts[10])

            path_of[node] = path
            hash_of[path] = node
            if kind in ("const", "extern"):
                const_bodies.add(node)
            if item != "-":
                implements[node] = item
                if not item_path.startswith("rustc_"):
                    external_impl.add(node)
        elif parts[0] == "specbound":
            spec_bounds.add(parts[1])
        elif parts[0] == "demand":
            demands[parts[1]].add((parts[2], parts[3]))
        elif parts[0] == "edge":
            if parts[3] == "construct":
                constructs[parts[1]].add(parts[2])
            else:
                edges[parts[1]].add(parts[2])
bodies = set(path_of)
impl_key = {node: key for node, key in impl_key.items() if key[0] not in spec_bounds}

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

functions = {}  # site -> (crate, path)
span_of = {}
for table in Path(args.sites).glob("*.sites"):
    for line in table.read_text(errors="replace").splitlines():
        f = line.split("\t")
        if len(f) >= 7 and f[1] == "cover":
            functions[f[0]] = (f[3], f[4])
            span_of[f[4]] = f[6]
external = set()
for u in args.external:
    external |= set(Path(u).read_text().split())

# Crates from crates.io that the scope takes in by name (rustc-hash, rustc-stable-hash): other
# dependencies, which the graph does not cover, can name their types and need their impls
# without any bound in the compiler saying so. Their impls are not gated on demand.
from_registry = {krate for _, (krate, path) in functions.items()
                 if not span_of.get(path, "compiler/").startswith(("compiler/", "src/", "library/"))}
impl_key = {node: key for node, key in impl_key.items()
            if path_of[node].split("::", 1)[0] not in from_registry}

# What programs outside the compiler ran of it: their mains call it, so it is a root.
external_roots = {hash_of[functions[x][1]] for x in external if x in functions and functions[x][1] in hash_of}
roots |= external_roots

# Rapid type analysis: a method of an impl for one of the compiler's types counts for trait
# dispatch (and as an external trait's root) only once reachable code builds the type; and a
# function of a trait impl only once reachable code needs that type to implement that trait (a
# call whose bounds say so, a cast to `dyn Trait`, the trait's item used with that `Self`). A
# call the caller's types resolve to the implementation reaches it directly.
reachable, live, demanded = set(), set(), set()
came_from = {}  # node -> (from node, how)
condition_from = {}  # condition -> the body that met it
waiting = defaultdict(set)  # condition -> functions waiting for it
queue = deque()


def missing(node):
    out = []
    t = self_type.get(node)
    if t is not None and t not in live:
        out.append(("live", t))
    k = impl_key.get(node)
    if k is not None and k not in demanded:
        out.append(("demand", k))
    return out


def offer(node, gated, source=None, how="root"):
    if node in reachable:
        return
    came_from.setdefault(node, (source, how))
    lacking = missing(node) if gated else []
    if lacking:
        waiting[lacking[0]].add(node)
    else:
        queue.append(node)


def satisfied(condition):
    for node in waiting.pop(condition, ()):
        offer(node, True)


for r in roots:
    offer(r, r in external_impl and r not in const_bodies and r not in external_roots)
while queue:
    node = queue.popleft()
    if node in reachable:
        continue
    reachable.add(node)
    for t in constructs.get(node, ()):
        if t not in live:
            live.add(t)
            condition_from[("live", t)] = node
            satisfied(("live", t))
    for k in demands.get(node, ()):
        if k not in demanded:
            demanded.add(k)
            condition_from[("demand", k)] = node
            satisfied(("demand", k))
    for nxt in edges.get(node, set()) | children.get(node, set()):
        offer(nxt, False, node, "edge")
        # A call to a trait item reaches the bodies implementing it.
        for impl in implementors.get(nxt, ()):
            offer(impl, True, node, "dispatch")
reachable_paths = {path_of[n] for n in reachable if n in path_of}
for target in args.why:
    node = hash_of.get(target)
    print(f"\nwhy {target}:" + ("" if node in reachable else " not reachable"))
    seen = set()
    while node is not None and node in reachable and node not in seen:
        seen.add(node)
        source, how = came_from.get(node, (None, "?"))
        extra = ""
        if True:
            for c in [("live", self_type.get(node)), ("demand", impl_key.get(node))]:
                if c[1] is not None and c in condition_from:
                    extra += f" [{c[0]} from {path_of.get(condition_from[c], condition_from[c])}]"
        print(f"   {path_of.get(node, node)}  <- {how}{extra}")
        node = source

functions = {s: v for s, v in functions.items() if v[0] not in NOT_AT_RUN_TIME}
paths = {path for _, path in functions.values()}
known = paths & set(hash_of)
unreach = {path for path in known if path not in reachable_paths}
print(f"{len(bodies)} bodies in the graph, {len(roots)} roots, {len(reachable & bodies)} reachable")
print(f"{len(functions)} instrumented functions; {len(known)} in the graph; "
      f"{len(unreach)} unreachable ({100 * len(unreach) / max(len(known), 1):.1f}%)")

hit = set(external)
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
                                           "ran_unreachable": sorted(hit_paths & unreach),
                                           "reachable_not_hit": sorted((paths - unreach) - hit_paths)}, indent=0))

if args.gaps:
    files = defaultdict(list)
    for site, (krate, path) in functions.items():
        if path not in unreach and path not in hit_paths:
            span = span_of.get(path, "?")
            files[(krate, span.rsplit(":", 2)[0])].append((span, path))
    with open(args.gaps, "w") as out:
        out.write(f"# Reachable functions that never ran: {sum(map(len, files.values()))}\n\n")
        by_crate = defaultdict(int)
        for (krate, _), fs in files.items():
            by_crate[krate] += len(fs)
        for krate in sorted(by_crate, key=lambda k: -by_crate[k]):
            out.write(f"## {krate} ({by_crate[krate]})\n\n")
            for (k, file), fs in sorted(files.items(), key=lambda kv: -len(kv[1])):
                if k != krate:
                    continue
                out.write(f"### {file} ({len(fs)})\n\n")
                for span, path in sorted(fs):
                    out.write(f"- `{path}` {span.rsplit(':', 2)[-2] if ':' in span else ''}\n")
                out.write("\n")
    print(f"gaps written to {args.gaps}")

#!/usr/bin/env python3
"""Generate docs/motivating.md from bugs.json and the outputs in out/."""
import json
from pathlib import Path

here = Path(__file__).resolve().parent
bugs = json.loads((here / "bugs.json").read_text())

STATUS = {b["issue"]: "reproduced" for b in bugs}
STATUS[111227] = ("partly reproduced: `foo.py` is missing from the dep-info before the fix and "
                  "present after it, but the stale metadata and ICE the issue describes did not "
                  "appear here")

# What this run showed, from out/<issue>/{before,after}.txt.
THIS_RUN = {
    34902: "before: 5 builds, 5 different rlibs (only the metadata member differs); identical with ASLR off. After: 5 identical",
    45841: "before: the .rmeta is rewritten in place (same inode), and 2 of 1,704 concurrent readers hit the issue's ICE in leb128.rs. After: replaced by rename, 0 of 1,712",
    65036: "before: two builds differ at byte 5163, re-exports in a different order. After: identical",
    68149: "before: the consumer opens and records the dependency's in-flight .rlib. After: only the .rmeta files",
    40364: "before: changing the variable leaves the binary printing the old value, and dep-info has no env-dep. After: the new value, and `# env-dep:MIRTH_DEMO=two`",
    82920: "before: the incremental pass-2 binary panics (`left: 2, right: 1`) while a clean build of the same source passes. After: both pass",
    84252: "before: the second incremental build ICEs with unstable fingerprints for `has_global_allocator`. After: both builds succeed",
    66955: "before: after changing the remap, the rlib still holds 2 copies of the old path. After: only the new path",
    89598: "before: the incremental pass-2 binary calls the wrong method (`left: 42, right: 17`); the clean build passes. After: both pass",
    107001: "before: a failed compile leaves `.rcgu.o` files in the output directory. After: none",
    111227: "before: the visualizer file is missing from dep-info. After: listed. The stale metadata did not appear here",
    111295: "before: the rebuilt binary still embeds the old visualizer script. After: the new one",
    117254: "before: a write error is swallowed (exit 0), a 1 MiB truncated .rmeta is published, and a dependent ICEs. After: an error and exit 1, but the truncated file is still published",
    119456: "before: an error and exit 1, yet the truncated .rmeta is published and a dependent ICEs. After: nothing is published, and the dependent gets `can't find crate`",
    122859: "before: E0277, the implied bound lost across crates. After: compiles",
    130201: "before: the dependent ICEs (`coroutine_by_move_body_def_id` unsupported by its crate). After: compiles",
    135514: "before: the incremental rebuild accepts overlapping impls and runs, while a clean build gives E0119. After: both give E0119",
    138678: "before: 8 builds, 8 different .rmeta files. After: 8 identical",
    139407: "before: after a failed build is fixed back, the rebuilt binary still contains the failed session's code and panics. After: it passes",
    139899: "before: each failing doctest leaves a `rustdoctest*` directory (4 left). After: none",
    114669: "before: an unchanged rebuild re-encodes the metadata (new inode). After: reused (same inode, hard-linked to the work product)",
    144004: "before: rustdoc shows neither attribute on the re-exports. After: `no_mangle` and `link_section` shown",
    140413: "before: 15 threaded builds give 2 different binaries. After: 15 identical",
    159677: "before: adding an unrelated library to the search path changes the .rmeta (differs at byte 1256). After: identical",
    150451: "before: 15 threaded builds give 10 different rlibs. After: 15 identical",
    129094: "before: 20 threaded builds give 4 different rlibs. After: 20 identical",
    162901: "before: the incremental build prints the error 4 times, a clean build once. After: once each",
}

# The fixes replayed by reverting them on the pinned compiler (docs/regressions.md).
MIRTH = {
    122859: "caught when the fix (#122891) is reverted: the list shows the table no longer written",
    130201: "caught when the fix is reverted: the build ICEs",
    138678: "caught when the fix is reverted: P5, P5 with threads, the touch rebuild and P6",
    144004: "missed when the fix (#144050) is reverted: only rustdoc reads these attributes",
    114669: "with #143247 reverted (metadata depending on a node that is never green), the touch rebuild's record catches it",
    129094: "not replayed (the revert does not apply cleanly)",
    159677: "not replayed (needs a decoy crate in the search path)",
}

PROPS = [
    ("P1", "An `.rmeta` reaches its final path only by a rename of a fully written file",
     "A reader that opens a half-written or truncated `.rmeta` misreads it, usually as an ICE "
     "in the decoder. rustc writes the metadata to a temporary directory and renames it into "
     "place; P1 checks that protocol on every process. The first bug below is where the "
     "protocol came from; the other two published a truncated file through the rename."),
    ("P2", "No process opens a dependency's `.rmeta` before it has been renamed into place",
     "With pipelining, a dependent starts as soon as its dependency's metadata exists, so the "
     "order of writes and opens across processes matters. P2 compares the timestamps of opens "
     "in readers with renames in writers."),
    ("P3", "A dependent reads only table entries the dependency wrote",
     "An entry the writer never wrote reads as a default, without complaint, so a table that "
     "stops being written shows up as wrong behaviour far away: a missing bound, a missing "
     "attribute, or an ICE in the reader. The `written` column of the blessed list records it."),
    ("P4", "Encoding reads no untracked state",
     "Environment variables, the clock, randomly seeded maps and files that are not declared "
     "inputs all reach outputs without incremental compilation or Cargo knowing. P4 records "
     "such reads while encoding."),
    ("P5", "Two clean builds give the same bytes",
     "Nondeterminism in metadata breaks reproducible builds and makes crate hashes, and so "
     "everything downstream, depend on chance."),
    ("P5t", "Two clean builds with `-Zthreads` give the same bytes",
     "The parallel front end adds a new source of nondeterminism: the order in which threads "
     "create and intern things."),
    ("P6", "An incremental rebuild gives the same results as a clean build",
     "Incremental compilation is only correct if what it reuses is what it would have "
     "computed. When it is not, the result is a stale output, a miscompilation or a wrong "
     "diagnostic, which `cargo clean` fixes. These bugs are why P6 compares an incremental "
     "rebuild with a clean build of the same source."),
    ("P7", "Nothing is left behind in the output directory",
     "Leftover temporary files waste space and can be picked up by later builds."),
    ("tracked", "Cross-crate reads are tracked, and metadata is reused when nothing changed",
     "Every query that reads another crate's metadata must record a dependency on that crate, "
     "or a later session reuses a stale result. The `tracked` column of the blessed list "
     "records it per query; the touch-only rebuild records whether metadata was reused."),
]

out = ["""# The bugs behind each property

mirth's properties were designed from first principles: what must hold for rustc's
handling of metadata and incremental compilation to be correct. This document records the
real rust-lang/rust bugs that violated each one, so a maintainer can see why a check exists.
Each bug was **reproduced on a toolchain from before its fix and shown fixed on one after**,
with no mirth involved: the original bug, as users met it.

The bugs were found by searching rust-lang/rust for each property; every issue and PR was
read, and every reproduction run on this machine (Linux x86_64). Reproduce one with
`docs/motivating/run.py <issue>`; the outputs used here are in `docs/motivating/out/`.

Some bugs appear under two properties. Three found by mirth itself
([`hunt/`](hunt)) are listed under P6 at the end.
"""]
for key, title, why in PROPS:
    rows = [b for b in bugs if key in b["properties"]]
    out.append(f"## {key}: {title}\n\n{why}\n")
    out.append("| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |\n|---|---|---|---|---|---|")
    for b in rows:
        out.append(f"| [#{b['issue']}](https://github.com/rust-lang/rust/issues/{b['issue']}) {b['title'].split(' (')[0][:80]} "
                   f"| [#{b['fix_pr']}](https://github.com/rust-lang/rust/pull/{b['fix_pr']}) | {b['merged']} "
                   f"| `{b['before_toolchain']}` → `{b['after_toolchain']}` | {STATUS[b['issue']].split(':')[0]} "
                   f"| {MIRTH.get(b['issue'], '—')} |")
    out.append("")
    for b in rows:
        out.append(f"**#{b['issue']}.** {b['how_it_violates']}\n")
        if STATUS[b["issue"]] != "reproduced":
            out.append(f"*Status:* {STATUS[b['issue']]}.\n")
        out.append(f"*This run:* {THIS_RUN[b['issue']]}. Outputs: [`before`](motivating/out/{b['issue']}/before.txt), [`after`](motivating/out/{b['issue']}/after.txt).\n")
        out.append(f"*Expected, from the issue and the research:* {b['observe']}\n")
out.append("""## Found by mirth (P6)

| bug | reproduce | cause | since |
|---|---|---|---|
| `Generics::param_def_id_to_index` order changes on each round trip through the incremental cache | `docs/hunt/repro.sh` (p6-generics) | an `FxHashMap` encoded in iteration order | at least 1.95 |
| a string literal encoded twice after an incremental rebuild | `docs/hunt/repro.sh` (p6-literals) | literals deduplicated when created but not when decoded | 1.90 (#116707) |
| the previous session's metadata republished after an edit that moves no span | `docs/hunt/repro.sh` (stale-source) | source map file hashes and lengths not tracked | 1.90 (#114669) |

Each reproduces on the official nightly and on stable 1.98.1, and has a draft report, a
candidate fix and a regression test in [`hunt/`](hunt).

## Not covered here

The 29 properties in [`properties.md`](properties.md) cite the bugs that suggested them,
checked against the issue text, but those bugs have not been reproduced this way.
""")
(here.parent / "motivating.md").write_text("\n".join(out))
print("written", sum(len(x) for x in out))

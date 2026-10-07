# Scaling up: history replay, an edit fuzzer, and a survey of properties

Three ways to find more bugs than one fixture and ten hand-written edits can:

1. **Replay real history.** `rustc/replay.py` walks a crate's git history, oldest
   first, building each commit incrementally on top of the last and again from scratch,
   and compares them (P6). Ten crates, up to 2,000 commits each.
2. **Fuzz edits on a large fixture.** `fixtures/sink` is a five-crate workspace with as
   many stable language features as fit. `rustc/fuzz.py` makes random mechanical edits to it
   and checks every incremental rebuild against a clean build.
3. **Survey past bugs for properties.** Agents read 1,000 fixed rustc bugs and extracted
   the invariants they violated; a second pass checked every citation. The result is
   [`properties.md`](properties.md).

All of it runs against a compiler with the candidate fixes for the bugs already known
([`hunt.md`](hunt.md)), so those do not drown new ones; each new finding is then confirmed
on the official nightly.

## What was found

| finding | how | status |
|---|---|---|
| Incremental rebuilds republish stale metadata when an edit moves no span | fuzzer and replay, independently | **new**; root-caused, regression from #114669 (1.90); fixed and tested ([report](hunt/issue-stale-metadata-reuse.md)) |
| The candidate fix for the literal bug over-deduplicated | replay (serde, one commit) | my own mistake; the fix was narrowed ([report](hunt/issue-literal-dedup.md)) |

**Stale metadata.** Since #114669, an incremental session reuses the saved `.rmeta` when
its dep-node is green. But the metadata's source map records every source file's length,
line table and content hash, read straight from the session's source map, which nothing
tracks. An edit that changes no query result (a comment after the last item, a typo fixed
inside a comment without moving any span, a change inside `#[cfg]`'d-out code) leaves the
node green, and the previous session's metadata is republished. A dependent then cannot match
the dependency's source: its diagnostics lose the snippet. Real histories hit it on ordinary
commits: memchr (5), serde, smallvec (2), hashbrown (1), anyhow (3), regex (several), all on
comment or docstring edits. The fix makes the metadata task read an `eval_always` query
fingerprinting the local source files, so reuse still happens when the files are truly
unchanged (the blessed touch-only rebuild still shows it).

**A flaw in my own fix.** The first fix for the literal bug deduplicated every immutable
allocation on decode. The serde replay showed P6 failing on commit `2f58a20` with it applied:
`-Zmeta-stats` put the whole 59-byte difference in `interpret-alloc-index`, because
allocations a clean session keeps apart were merged. The fix now records, at encode time,
whether an allocation was created through deduplication, and repeats only that. The same
replay window is clean with it.

With all three fixes, each regression test passes and each fails without its own fix, and
rustc's incremental, metadata UI and run-make, consts/statics/const-generics UI and
codegen-llvm tests pass.

## History replay

`rustc/replay.py --rustc <rustc> --repo <checkout> --work <dir> [--commits N] [--from i --to j]`

For each first-parent commit, oldest first: check it out, `cargo build --lib`
incrementally on the previous commit's target directory, then build the same source from
scratch at the same path. Registry dependencies are not compiled incrementally by Cargo, so
the clean build starts from a copy of the target directory with every path package cleaned
(`cargo clean -p`) and the incremental cache removed. It compares every `.rmeta` Cargo
reports for a path package, and flags ICEs, builds where only one side fails, and clean
builds that did not actually recompile ("stale", which would compare a file with itself).
Every commit is built with `--cap-lints=warn`, so old commits that deny warnings still build.

Crates: regex, itertools, smallvec, hashbrown, indexmap, memchr, bitflags, serde, anyhow,
thiserror. REPLAY_TABLE

A positive control: on the unpatched nightly, every incremental rebuild of itertools'
last 12 commits differs from a clean build (the `param_def_id_to_index` bug); on the patched
compiler none do.

Harness problems found and fixed on the way, each of which produced false findings or
skipped commits silently: artifacts of earlier commits counted as differences; path
dependencies that are not workspace members left uncleaned; `cargo clean` refusing a
directory without `CACHEDIR.TAG`.

## The fixture and the fuzzer

`fixtures/sink`, about 1,200 lines in five crates:

- `sink-macros`, a proc-macro crate with no dependencies: a derive with a helper
  attribute, an attribute macro, a function-like macro;
- `sink-core`, with a build script generating code and a cfg, features, traits with
  associated types and consts, generic associated types, blanket impls, trait objects and
  upcasting, operators, `impl Trait` and `async fn` in traits, async closures, const
  generics, const fn and const blocks, statics, thread locals, unions, `repr`, `Drop`,
  raw pointers, `MaybeUninit`, FFI, `#[track_caller]`, inline attributes, deprecation,
  error types, iterators, closures, patterns, labelled blocks, `let else`, `if let` chains,
  collections, `Rc`/`RefCell`/`Arc`/`Mutex`, scoped threads, exported and internal
  `macro_rules!`;
- `sink-mid`, using all three kinds of proc macro and the exported macros, with nested
  modules and restricted visibility, re-exports, and generic code dependents instantiate;
- `sink-dy`, built as a dylib too;
- `sink`, a binary that exercises everything and checks 60 results, so a miscompile fails.

`rustc/fuzz.py --rustc <rustc> --fixture fixtures/sink --work <dir> [--workers N]`

Each worker keeps one evolving copy of the fixture. It makes a random edit, chosen from 16
kinds (a comment, a blank line, an indented line, swapped or moved or deleted items, a
duplicated function, a new item of one of 12 shapes, a changed number or string, an
inline attribute, a doc comment, reordered derives, narrowed visibility, a renamed local),
and builds incrementally with `-Zincremental-verify-ich`. If the edit does not compile, it
is reverted; the next build then also exercises recovery from a failed session. Otherwise
the same source is built from scratch at the same path, and every `.rmeta` and every proc
macro's embedded metadata are compared (P6), the two binaries are run and their output
compared, and ICEs, hangs and one-sided failures are reported. Every 40 kept edits the worker
starts again from the pristine fixture. A finding keeps every edit since the last reset, and
`rustc/fuzz-replay.py` replays it exactly.

Throughput on this 16-core machine: about 2 edits a second with six workers, about
170,000 a day, while other work shared the machine. FUZZ_TABLE

Millions of edits means about a week here, or several machines.

## The survey

[`properties.md`](properties.md) has 29 properties plus the crash baseline, ranked by how
many of the 1,000 bugs violated each, after every citation was checked against the issue
text (107 of 156 citations held). The top: intern-time well-formedness of type-system values
(12 bugs), the old and new trait solvers agreeing (11), `extern "C"` lowering matching C
(9), MIR validating at every opt level on real crates (8), library soundness under injected
panics (7). The incremental and metadata properties mirth already checks rank lower by bug
count (3 and 1), yet they are where all four bugs found in this work came from, which says
more about which bugs get reported and fixed than about where bugs are.

Small models did both the reading and the checking. The issue lists are leads, not proof.

## Limits

- Everything ran on Linux x86_64, single-threaded unless stated.
- `-Zthreads` is not fuzzed: the fixture has `impl Trait` in traits, and #162202 makes
  every threaded build differ.
- The fuzzer edits text, not syntax trees; about 8% of edits do not compile and are
  reverted, and some kinds of edit (deleting items, narrowing visibility) almost never do.
- The replay compares `.rmeta` only; the binaries' code and debug info are not compared.

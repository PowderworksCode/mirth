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
| Three untracked options that change reused output | closed-bug queries and an option audit ([`ur-queries.md`](ur-queries.md)) | new instances of #66955's class; report drafted ([draft](hunt/issue-untracked-options.md)) |

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
thiserror. On the compiler with all three fixes:

| crate | commits | both built | P6 | ICE | one side failed |
|---|---:|---:|---:|---:|---:|
| anyhow | 669 | 524 | 0 | 0 | 0 |
| bitflags | 301 | 300 | 0 | 0 | 0 |
| hashbrown | 477 | 456 | 0 | 0 | 0 |
| indexmap | 465 | 415 | 0 | 0 | 0 |
| itertools | 1,358 | 942 | 0 | 0 | 0 |
| memchr | 267 | 267 | 0 | 0 | 0 |
| regex | 700 of 1,368 | 590 | 0 | 0 | 0 |
| smallvec | 391 | 390 | 0 | 0 | 0 |
| serde | 806 of 2,000 | 425 | 0 | 0 | 0 |
| thiserror | 556 | 553 | 0 | 0 | 0 |

4,862 commits built on both sides and none differed. Commits that failed on both sides are
mostly old code the current compiler rejects. regex and serde stopped part-way when the
disk filled. This run compared `.rmeta` only; the rlib and diagnostic comparisons were
added after it started.

Before the stale-metadata fix, the same replay found that bug on ordinary commits in
memchr, smallvec, hashbrown, anyhow and regex: comment and docstring edits that moved no
span. One P6 failure, on serde, came from my own first fix for the literal bug.

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
- `sink`, a binary that exercises everything and checks 66 results, so a miscompile fails.

Later, `sink_core::extras` added what the edit fuzzer cannot create on its own: statics
holding references and nested allocations, `include_str!`/`include_bytes!`, a `#[path]`
module, `#[no_mangle]` and `#[used]`, `cfg_attr`, `Any`, a glob re-export, an exported macro
using `$crate`, a proc macro with a `mixed_site` binding, and a generic `#[inline]` function
that `sink-mid` inlines from the metadata.

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

Since [`shadow-mode.md`](shadow-mode.md), the fuzzer and the replay also run every build
with the compiler's own check of what it reused (`RUSTC_VERIFY_REUSE`, on a compiler with
[`hunt/verify-reuse.patch`](hunt/verify-reuse.patch)), and report what it prints.

Throughput on this 16-core machine: about 2 edits a second with six workers, about
170,000 a day, while other work shared the machine.

| compiler | edits | built and compared | findings |
|---|---:|---:|---|
| with the first two fixes | 4,009 | 3,336 | 60 stale metadata reuses (the third bug) |
| with all three fixes | 10,717 | 8,936 | none |

The last 1,455 of those comparisons also checked object code, binaries and diagnostics.

Millions of edits means about a week here, or several machines.

## Other flags

Short fuzz runs (two workers, 40 edits each) with one flag set added to every build, on the
compiler with the three fixes, the reuse check and
[`hunt/debuginfo-checksum-stopgap.patch`](hunt/debuginfo-checksum-stopgap.patch):

| flags | compared | differences from a clean build |
|---|---:|---|
| `-Copt-level=2` (before the stopgap) | 102 | binaries and objects in a third of the rebuilds: [finding 6](hunt.md) |
| `-Copt-level=2` | 152 | none |
| `-Cinstrument-coverage` (official nightly) | 126 | none besides the known metadata bugs |
| `-Cdebuginfo=line-tables-only` | 71 | none |
| `-Cpanic=abort` | 68 | none |
| `-Zshare-generics=yes -Copt-level=1` | 71 | none |
| `-Copt-level=s` | 67 | none |
| `-Ccodegen-units=1 -Copt-level=3` | 69 | none |
| `-Zdwarf-version=5` | 66 | none |
| `-Csplit-debuginfo=unpacked`, `=packed` | 74, 67 | every rebuild, but two clean builds differ too: objects name their `.dwo` files with the session suffix, so this oracle does not apply |

The reuse check printed only the known allocation-sharing pattern in all of them.

**Threads.** With `-Zthreads=8`, two clean builds of `fixtures/sink` already differ
(#162202: the definitions made for `impl Trait` and `async fn` in traits get indices in a
nondeterministic order). [`fixtures/sink-threads.patch`](../fixtures/sink-threads.patch)
turns the two such trait methods into boxed iterators and futures, and the free `async fn`s
into functions returning `impl Future` (once the fuzzer duplicated an `async fn`, clean
threaded builds of the crate differed six ways in six builds, as in #162202's first
example); with it, clean threaded builds agree. The fuzzer now builds clean a second time
whenever anything differs, and reports P5 rather than P6 if the two clean builds disagree.

That still left `-Zthreads` fuzzing finding #162202 again whenever an edit made another
`impl Trait` or `async fn`. [`hunt/threads-def-order-stopgap.patch`](hunt/threads-def-order-stopgap.patch),
a testing aid like the other stopgap, makes the definitions that queries create (RPITIT
associated types, captured lifetimes of opaque types, coroutine by-move bodies) on one
thread, in definition order, just before rustc's `commit_end_of_determinism`, when the
front end is parallel. With it, eight `-Zthreads=8` builds of #162202's reproduction give one
metadata file instead of four, and threaded builds of the original `fixtures/sink` agree,
so the threaded fuzzer runs on the unmodified fixture. With only one of the two changed, clean builds agreed but incremental rebuilds
differed from clean ones about half the time: the same out-of-order indices, made in one
session and kept by the next (the incremental tables for the made-up associated types ended
two indices earlier). That is #162202 reaching incremental sessions, not a new bug.

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

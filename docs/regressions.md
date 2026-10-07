# Seven past rustc bugs, replayed

The seven edits in [`results.md`](results.md) were written knowing what
mirth watches. This is the stronger test: real bugs from rustc's history,
each fixed in a merged PR. For each one, the fix was reverted on the pinned
compiler (`ea137335b`), the instrumented compiler was rebuilt, and
`rustc/check.sh chain` was run, along with the same selection of rustc's
own tests as for the edits (180 incremental, 532 UI, 46 run-make).

The patches are in [`rustc/regressions/`](../rustc/regressions), the full
output of each run is in [`regressions/`](regressions), and
`EDITS=regressions rustc/edits.sh chain` reproduces it.

## Result

| PR | the bug | mirth | rustc's tests |
|---|---|---|---|
| [#122891](https://github.com/rust-lang/rust/pull/122891) | a trait's implied predicates were not encoded, so dependents lost bounds | **caught**: the list | pass |
| [#130201](https://github.com/rust-lang/rust/pull/130201) | an async closure's by-move body was not encoded; dependents ICE | **caught**: the build ICEs | pass |
| [#138678](https://github.com/rust-lang/rust/pull/138678) | unused doc link reference definitions were encoded in hash order | **caught**: P5, P5 with threads, the touch rebuild, P6 | pass |
| [#143247](https://github.com/rust-lang/rust/pull/143247) | metadata always depended on a node that is never green, so it was never reused | **caught**: the touch rebuild | pass |
| [#82047](https://github.com/rust-lang/rust/pull/82047) | the old `.rmeta` was not removed before the rename (slow on ext4) | **caught**: the list | pass |
| [#144050](https://github.com/rust-lang/rust/pull/144050) | `#[no_mangle]` and `#[link_section]` were not encoded for rustdoc | missed | pass |
| [#162910](https://github.com/rust-lang/rust/pull/162910) | `DefPathHashMap` was encoded in insertion order under the parallel front end | missed | pass |

mirth catches five of the seven. The selected rustc tests catch none.
That is less damning than it looks: each fix added a regression test, and
none of those tests is in the selection (`tests/ui/associated-type-bounds`,
`tests/ui/async-await/async-closures`, `tests/rustdoc-html`, and so on).
The selection was made for the edits, before the regressions were chosen.
Run whole, rustc's test suite would catch most of these today, because of
those very tests. The question is what would have caught them before.

## What it took

Two were caught by the fixture and the checks as they stood: #122891
(the chain fixture already has a public trait) and #82047. The other three
needed something added, each written knowing the bug:

- **A step.** #143247 was caught by a new step in `check.sh`: an incremental
  rebuild after touching every source file, whose record is blessed in
  `tests/rmeta/chain.touch.txt`. With nothing changed, rustc should reuse
  each crate's metadata from the incremental cache, and the record shows it
  doing so. The run-make test
  `metadata-stub-incremental-reuse` takes the same path and passed: it
  checks that the build works, not that the metadata was reused.
- **Fixture code.** #138678 needs a doc comment with unused reference
  definitions; `mid` now has one. #130201 needs an async closure called by
  value in another crate; `base` and `app` now have one.

So the fair reading is: the checks are general, but a check can only see a
bug the fixture exercises. Every addition stays in the fixture, so each is
now a regression test for its bug, as rustc's own test for it is.

## What each looks like

**#122891.** Reverting the fix stops `base` from writing
`explicit_implied_clauses_of`, and dependents fall back to the super
clauses:

```diff
-       4     2 items     2 tracked    2 written  base               explicit_implied_clauses_of
+       6     2 items     2 tracked               base               explicit_implied_clauses_of
+       4     2 items     2 tracked    2 written  base               explicit_super_clauses_of
```

The fixture has no bound that only the implied predicates carry, so it
still compiles. The list shows the change anyway.

**#130201.** The dependent ICEs, as in the original issue:

```text
DefId(20:17 ~ base[f922]::greeter::{closure#0}::{closure#0}) does not have a "coroutine_by_move_body_def_id"
```

**#138678.** The unused reference definitions are resolved and encoded in
an order that changes from run to run, so two clean builds disagree (P5),
and so do an incremental rebuild and a clean build (P6). In the touch
rebuild, `mid`'s metadata is encoded again instead of reused, because its
hash changed.

**#143247.** In the touch rebuild, every library encodes its metadata
again instead of opening the incremental cache's copy:

```diff
-  open           target/debug/incremental/base-#/s-*/metadata.rmeta in encoder::encode_metadata
+  encode-to      target/debug/build/base/#/out/rmeta*/full.rmeta in encoder::encode_metadata
```

and the record of what was encoded appears where there was none.

**#82047.** The `remove_file` before each rename disappears from the
list. The bug was a performance one (ext4's `auto_da_alloc` flushes a file
renamed over an existing one), so nothing else fails.

## The two misses

**#144050** changes which attributes are encoded for rustdoc. The record
counts table writes per item, and `base_limit` still has its doc comment
encoded as an attribute, so the count does not change. Only rustdoc reads
these attributes, and mirth does not run rustdoc. Seeing it would take
recording what each table entry contains, or a `cargo doc` step.

**#162910** makes metadata depend on the order in which threads create
definitions. `check.sh` builds twice with `-Zthreads=8` and compares, and
with the fix reverted the builds matched. A follow-up run of 16 clean
builds with `-Zthreads=8` (`REPEAT=16 HUNT_EDITS=0 rustc/hunt.sh chain`)
matched too, and so did rustc's own
`tests/run-make/parallel-reproducible-build`, built ten times per input.
The fixture does not create definitions late enough, in parallel, for the
order to vary. Even if it did, a difference would be hard to pin on this
bug: on the unmodified compiler, 1 of 24 threaded builds of the chain
fixture already differed. That one was not traced; the open parallel
reproducibility issues in [`hunt.md`](hunt.md) are the likely cause.

## Limits

- Five of seven caught, with three fixture and step additions made knowing
  the bug. A fair estimate of what mirth would have caught, unprompted, at
  the time is the two caught without additions, plus whatever a broader
  fixture would have exercised.
- One fixture. Bugs in code a fixture does not exercise are invisible.
- The candidates came from a search of rustc's history for metadata and
  incremental fixes that still revert cleanly. Several good ones did not:
  #161450 (syntax context encoding under threads) needs a port, and
  #117301 (a write error swallowed while encoding) needs fault injection.

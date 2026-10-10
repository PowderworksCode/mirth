# New solver: long iterator chains take 10× longer and 4× the memory since nightly-2026-08-04

Facts for finding 32. Found by the scaling check (`mirth-lab scale-check`, shape `iter-chain`:
growth exponent of compile time 2.6–3.5 at N ≤ 100).

## What happens

[`tests/iter-chain-200.rs`](tests/iter-chain-200.rs) is one statement:
`(0u64..10).map(|x| x.wrapping_add(0)) … .map(|x| x.wrapping_add(199)).sum()`, 200 `map`
calls. User time and peak memory of `rustc iter-chain-200.rs` (-Copt-level=0):

| toolchain | default | `-Znext-solver=coherence` (old solver) | `-Znext-solver=globally` |
|---|---|---|---|
| 1.80.0 | 5.7 s, 115 MB | | |
| 1.90.0 | 7.0 s, 125 MB | | |
| 1.98.0 | 2.3 s, 118 MB | | |
| nightly-2026-07-18 | 2.2 s, 119 MB | 2.2 s, 118 MB | 4.6 s, 517 MB |
| nightly-2026-08-03 | | | 4.5 s, 519 MB |
| nightly-2026-08-04 | | | 37.3 s, 2.42 GB |
| nightly-2026-10-06 | 53.4 s, 2.04 GB | 6.1 s, 131 MB | 52.8 s, 2.04 GB |

At the mirth pin (default solver): N=50 0.19 s, N=100 1.2 s, N=200 68.6 s and 2.0 GB.
`-Ztime-passes` puts 65 of 73 s in `type_check_crate`. From nightly-2026-08-04 the build also
warns once, "overflow evaluating the requirement `Map<Map<…<Map<_, {closure@…}>…>>: Iterator`"
("this was previously accepted by the compiler but is being phased out"); earlier nightlies
do not warn.

Two separate changes show in the table: the new-solver slowdown between 2026-08-03 and
2026-08-04 (this finding), and the default switching to the new solver between July and
October ([`solver.md`](../solver.md)). The old solver also went from 2.2 s to 6.1 s over the
same period; not bisected.

## Bisection

Over nightlies, `-Znext-solver=globally`, "bad" above 20 s user time: 2026-07-28 4.5 s,
2026-08-02 4.7 s, 2026-08-03 4.5 s, 2026-08-04 37.3 s, 2026-08-07 38.0 s, 2026-08-27 58.4 s.

- last good: nightly-2026-08-03, 11177f2235f0c842b00f82c558ad9480c0c3a895
- first bad: nightly-2026-08-04, 504869653f510b279c542e65ccd1ea9710c119ba

The range has four merges. One touches the trait solver: rollup #160414, containing #160254
("fix-fcw-missing", commit 1489e477b62 "rerun even if the goal has ty vars"). Not confirmed by
building a compiler with it reverted.

## Where

`compiler/rustc_next_trait_solver/src/solve/eval_ctxt/mod.rs`,
`maybe_evaluate_root_goal_with_higher_recursion_limit` (and the proof-tree variant). #160254
removed this early return:

```rust
    // Some goals no longer overflow after the stalled infers are resolved.
    // Thus we don't have to rerun eagerly here.
    let has_stalled_infers = match predicate.kind().skip_binder() { … };
    if has_stalled_infers {
        return;
    }
```

so a root goal that overflows while it still has inference variables is now re-evaluated with
twice the recursion limit (to decide whether to emit the overflow future-compat warning). The
goal in the warning has one (`Map<_, …>`, innermost). Whether the rerun happens once per
fulfillment iteration, which would fit the growth with N, was not checked.

## Scope

Any method chain long enough that the solver overflows on the receiver's trait goal while its
innermost type is still being inferred. 200 is long for hand-written code; generated code and
builder- or iterator-heavy macros reach it.

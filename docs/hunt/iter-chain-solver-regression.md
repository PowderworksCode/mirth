# New solver: rejecting an over-long iterator chain takes 10× longer and gives 75 errors

Facts for finding 32. Found by the scaling check (`mirth-lab scale-check`, shape `iter-chain`),
which timed out at N=200. Reproduce with
[`repro/32-iter-chain-solver-regression.sh`](repro/32-iter-chain-solver-regression.sh).

**Correction (2026-10-10).** The first version of this finding reported a compile-time
regression on valid code. That was wrong: the 200-map program is over the recursion limit and
is rejected by every toolchain; the earlier timings did not check the exit status. Programs
under the limit are not slower under the new solver (it is faster there). What changed is how
the rejection behaves.

## What happens

`(0u64..10).map(|x| x.wrapping_add(0)) … .sum()` with N `map` calls
([`tests/iter-chain-200.rs`](tests/iter-chain-200.rs) is N = 200). From N = 128 the
`Map<Map<…>>` type exceeds the default recursion limit and every toolchain rejects the program.

| N | toolchain (solver) | result | user time, peak memory |
|---|---|---|---|
| 126 | 1.98.0, nightly-2026-07-18 (old) | compiles | 3.3–3.6 s |
| 126 | nightly-2026-10-06 (new, default) | compiles | 1.9 s |
| 126 | nightly-2026-08-03 / -08-04, `-Znext-solver=globally` | compiles, one overflow warning | 2.4 s / 2.5 s, 425 MB |
| 200 | 1.98.0, nightly-2026-07-18 (old) | E0275 "overflow evaluating the requirement `Map<…>: Iterator`", 1 error | 2.2 s, ~120 MB |
| 200 | nightly-2026-08-03, `-Znext-solver=globally` | rejected, 1 error, 2 overflow warnings | 4.9 s, 507 MB |
| 200 | nightly-2026-08-04, `-Znext-solver=globally` | rejected, **74 × E0320** ("overflow while adding drop-check rules for `Map<…>`") and 219 overflow warnings | **40.4 s, 2.37 GB** |
| 200 | nightly-2026-10-06 (new, default) | the same as 08-04 | ~53 s, 2.0 GB |

So since nightly-2026-08-04, under the new solver (nightly's default), rejecting the program
costs about 8× the CPU time and 4.7× the memory, and reports 75 error lines and 219 warnings
where older compilers report one error. The overflow warnings are the future-compatibility lint
"this was previously accepted by the compiler but is being phased out".

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

Programs that are rejected anyway: a method chain over the recursion limit (here from 128
calls). The cost is a slow, noisy error (tens of seconds and gigabytes, 75 errors instead of
one) rather than wrong acceptance or a slower valid build. Generated code and macros that build
long chains are where a user would meet it.

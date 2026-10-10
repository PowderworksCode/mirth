# `-Zunleash-the-miri-inside-of-you`: an incremental rebuild drops its warning and its feature-gate error

Facts for finding 58. Found by `mirth-lab ui-incr` (P6 over the UI corpus,
[`checks.md`](../checks.md), fourth batch) on `consts/const-eval/const_fn_ptr.rs` and
`const_fn_ptr_fail.rs`: after an edit, the incremental rebuild lacks the "skipping const checks"
warning a clean build of the same source prints.

## What happens

With `-Zunleash-the-miri-inside-of-you`, const checking does not reject operations it would
otherwise reject; it records each one in `Session::miri_unleashed_features`
(`rustc_session/src/session.rs`, from `rustc_const_eval/src/check_consts/check.rs:271`). At the
end of the session, `check_miri_unleashed_features` prints the warning "skipping const checks",
with one note per recorded operation, and when one of them skipped a feature gate and nothing
else failed, the error "`-Zunleash-the-miri-inside-of-you` may not be used to circumvent feature
gates, except when testing error paths in the CTFE engine".

The list is session state filled as a side effect of const checking, not part of a query result
or a recorded side effect. When an incremental rebuild reuses const checking from the cache, it
does not run, nothing is recorded, and neither the warning nor the error is emitted.

| `const_fn_ptr.rs` (run-pass, `-Zunleash-the-miri-inside-of-you`) | clean | rebuild, unchanged | rebuild, a blank line added | clean of the edited file |
|---|---|---|---|---|
| nightly-2026-10-06 | warning | none | none | warning |
| 1.98.0 | warning | none | none | warning |
| 1.80.0 | warning | none | none | warning |

[`tests/unleash-gate-incremental.rs`](tests/unleash-gate-incremental.rs) (a `const fn` calling
`AtomicUsize::fetch_add`, a `const_atomic`-gated const fn), `--crate-type lib
-Zunleash-the-miri-inside-of-you -Cincremental=inc --emit=metadata`, `RUSTC_BOOTSTRAP=1`:

| nightly-2026-10-06 | exit |
|---|---|
| clean build | 1 (the feature-gate error) |
| rebuild, unchanged | 0 |
| rebuild, a blank line added | 0 |
| clean build of the edited file | 1 |

So the rebuild accepts a crate the clean build rejects. On 1.98.0 and 1.80.0 `fetch_add` is not a
gated const fn, so this file shows only the lost warning there.

## Scope

Only builds with `-Zunleash-the-miri-inside-of-you`, a flag for testing the const evaluator
(tests/ui/consts/miri_unleashed). Low severity; the same shape as the untracked options of
[`untracked-reads.md`](../untracked-reads.md): state that decides a diagnostic, outside the
dependency graph.

Searches of rust-lang/rust issues for the warning and the flag with "incremental" found nothing
(2026-10-10).

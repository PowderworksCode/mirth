# Finding 17: the "skipping const checks" warning disappears on an incremental rebuild

Facts for a report; the report itself is for a person to write (rust-lang/rust's LLM policy).

**Repro.** `tests/ui/consts/const-eval/const_fn_ptr_fail.rs` from rustc's own suite:

    rustc const_fn_ptr_fail.rs -Zunleash-the-miri-inside-of-you -Cincremental=inc   # warns
    rustc const_fn_ptr_fail.rs -Zunleash-the-miri-inside-of-you -Cincremental=inc   # no warning

**Expected.** Both builds print `warning: skipping const checks` (the test expects it:
`//~? WARN skipping const checks`), as a build with a fresh incremental directory does.

**Actual.** Only the first. The rebuild prints nothing, with or without an edit to the file
(changing `x * 2` to `x * 3` in `double` gives the same).

**Versions.** 1.60.0, 1.78.0, 1.90.0, 1.98.1 and nightly-2026-10-06 (with `RUSTC_BOOTSTRAP=1`
where needed).

**Cause, as far as followed.** Const checking records each feature it lets through under the
option on the session (`Session::miri_unleashed_feature`), and the warning is emitted from that
list at the end of the session. A rebuild that takes const checking's results from the
incremental cache records nothing, so there is nothing to warn about: a side effect of a query
that is not replayed, the same kind as finding 7. The option is for testing the compiler.

**How mirth found it.** `rustc/ui-fuzz.py` over rustc's UI tests ([`coverage.md`](../coverage.md)):
the first edit to this test, its rebuild's diagnostics compared with a clean build's.

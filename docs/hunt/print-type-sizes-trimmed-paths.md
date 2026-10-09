# Finding 11: an incremental rebuild ICEs after a session with -Zprint-type-sizes

Facts for a report; the report itself is for a person to write (rust-lang/rust's LLM policy).

**Repro.** `docs/hunt/repro.sh`, "print-type-sizes-ice":

    cat > lib.rs <<'RS'
    pub async fn inner() -> u32 { 1 }
    pub async fn outer() -> u32 { let s = String::from("x"); inner().await + s.len() as u32 }
    pub fn make() -> impl std::future::Future<Output = u32> { outer() }
    RS
    rustc --edition 2021 --crate-type lib -Cincremental=inc -Zprint-type-sizes lib.rs
    echo 'pub fn g() {}' >> lib.rs
    rustc --edition 2021 --crate-type lib -Cincremental=inc lib.rs

**Expected.** The second build succeeds, as a clean build of the edited file does.

**Actual.** `thread 'rustc' panicked at compiler/rustc_errors/src/lib.rs:458:17:`
"`trimmed_def_paths` called, diagnostics were expected but none were emitted". Exit 101.

**Conditions.** The edit is needed (without it every node stays green and nothing reruns); any
edit that changes HIR does. The second session must not have `-Zprint-type-sizes`,
`-Zquery-dep-graph`, `-Zdump-mir`, `-Zunpretty` or `RUSTC_LOG`, which exempt a session from
the check. Needs a coroutine that awaits another future.

**Versions.** With `RUSTC_BOOTSTRAP=1`: fine on 1.60.0 through 1.78.0, panics on 1.79.0
through 1.98.1 and nightly-2026-10-06.

**Cause.** `variant_info_for_coroutine` in `rustc_ty_utils/src/layout.rs` formats the type of
an `__awaitee` field with `field_layout.ty.to_string()`, which uses trimmed paths, inside the
`layout_of` query (via `record_layout_for_printing`, under `-Zprint-type-sizes`). The type of
the whole layout is formatted under `with_no_trimmed_paths!`, this field type is not. In the
first session `Session::record_trimmed_def_paths` returns early because of
`-Zprint-type-sizes`, and the dependency graph records `layout_of -> trimmed_def_paths`. In
the next session, `try_mark_green` for a body query reaches that edge; `trimmed_def_paths`
has a changed input (the edit) and is executed, and without the exemption it sets
`must_produce_diag`, which panics at the end of the session. Backtrace:
`set_must_produce_diag` ← `record_trimmed_def_paths` ← `trimmed_def_paths` ←
`try_execute_query` ← `try_mark_previous_green` ← `ensure_can_skip_execution` ←
`par_hir_body_owners` (`run_required_analyses`).

`-Zprint-type-sizes` is `[UNTRACKED]`, so the second session reuses the first's graph.

**How mirth found it.** The three-way walk over untracked option transitions on
`fixtures/sink` (`docs/flags.md`): 37 of 345 rows, all with `-Zprint-type-sizes=yes` before
and without it after. A delta minimization left `-Zprint-type-sizes` before and
`-Zvalidate-mir` after; on sink no edit is needed; which input changes there was not
followed.

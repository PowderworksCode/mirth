# rustc's internal checks on the UI tests

Facts for findings 21 to 24. Found by `mirth-lab crash-diff` ([`checks.md`](../checks.md), check 19):
every standalone UI test (18,624) compiled with the release compiler under test and again with a
compiler built from the same tree with `rust.debug-assertions = true`,
`rust.debug-assertions-std = true` and `rust.overflow-checks = true`, plus `-Zvalidate-mir`.
17 tests pass the release compiler and fail the internal checks. They fall into four groups.

Why rustc's CI does not see them: compiletest does not pass `-Zvalidate-mir` to UI tests (groups
21 and 24), and it pins `-Znext-solver=coherence`, while nightly's default is the new solver
everywhere ([`solver.md`](../solver.md)). Groups 22 and 23 pass under the pinned solver.

| finding | tests | fires with | release + `-Zvalidate-mir` | debug assertions alone | debug assertions + old solver |
|---|---:|---|---|---|---|
| 21 | 10 | `-Zvalidate-mir` | ICE | passes | ICE |
| 22 | 5 | debug assertions and the new solver | passes | ICE | passes |
| 23 | 1 | debug assertions (overflow checks) and the new solver | passes | ICE | passes |
| 24 | 1 | `-Zvalidate-mir` | ICE | passes | ICE |

## 21. MIR validation: SIMD field projections, and an unsize coercion

`compiler/rustc_mir_transform/src/validate.rs:81`, after `LintAndRemoveUninhabited`.

Nine tests: "Projecting into SIMD type … is banned by MCP#838". The projections come from field
access on `#[repr(simd)]` structs and from derived impls on them (`{impl#1}::clone` in
`simd/shuffle.rs`, `{impl#10}::eq` in `simd/intrinsic/generic-select.rs`, a const in
`consts/const-eval/simd/insert_extract.rs`). Type checking accepts the code, and the validator
then rejects the MIR built from it.

- `simd/shuffle.rs`, `simd/monomorphize-shuffle-index.rs`, `simd/masked-load-store-build-fail.rs`
- `simd/intrinsic/generic-arithmetic-saturating-2.rs`, `generic-gather-scatter.rs`,
  `generic-select.rs`, `generic-shuffle.rs`, `inlining-issue67557-ice.rs`
- `consts/const-eval/simd/insert_extract.rs`

One test: `async-await/issue-86507.rs`, in `{impl#0}::bar`: "Unsize coercion, but
`Pin<Box<{async block}>>` isn't coercible to `Pin<Box<dyn Future<Output = ()> + Send + '_>>`".
Type checking accepted the coercion. The validator, in its own typing environment, cannot prove
it. This test is the regression test for #86507 (an earlier ICE).

## 22. New-solver assertion in region outlives

`compiler/rustc_trait_selection/src/regions.rs:37`:
`assertion failed: !infcx.next_trait_solver() || !type_outlives.has_non_rigid_aliases()`.
Reached only with debug assertions and the new solver (nightly's default):

- `pattern/usefulness/impl-trait.rs`
- `type-alias-impl-trait/implied_bounds2.rs`, `implied_lifetime_wf_check3.rs`,
  `implied_lifetime_wf_check4_static.rs`, `unbounded_opaque_type.rs`

Related: #160206 (closed), a sibling assertion in the same code path
(`!tcx.next_trait_solver_globally() || !(verify_if_eq.ty, test_ty).has_non_rigid_aliases()`).
This one was not found in the issue tracker.

## 23. Integer overflow in `Instance` resolution under the new solver

`compiler/rustc_middle/src/ty/instance.rs:421`: `attempt to add with overflow`, compiling
`recursion/issue-83150.rs` (which expects a recursion-limit error). Overflow checks on, new solver.
With overflow checks off (release builds) the addition wraps silently; what follows was not
investigated.

## 24. MIR validation: moving a dereferenced unsized place into a call

`validate.rs:444`: "encountered `Move` of a non-local, non-box place in `Call` terminator:
`_1 = udrop::<[u8]>(move (*_2))`", compiling `unsized-locals/unsized-exprs2.rs` (the incomplete
`unsized_fn_params` feature).

## Reproduction

```
rustc +<pinned nightly or rustc-verify12> tests/ui/simd/shuffle.rs -Zvalidate-mir           # 21
rustc +<debug-assertions build> tests/ui/type-alias-impl-trait/implied_bounds2.rs           # 22
rustc +<debug-assertions build> tests/ui/recursion/issue-83150.rs                           # 23
rustc +<pinned nightly> tests/ui/unsized-locals/unsized-exprs2.rs -Zvalidate-mir           # 24
```

Each with the test's own `//@ compile-flags` and edition. The debug-assertions build is
`./x.py build --stage 1 compiler/rustc library --set rust.debug-assertions=true --set
rust.debug-assertions-std=true --set rust.overflow-checks=true` (mirth: `~/mirth-work/build-da`).

## Local stopgap

None: these are checks failing, not wrong output, and they do not affect mirth's incremental
checks. `mirth-lab crash-diff` takes them as `--known` (`rustc/crash-known.txt`).

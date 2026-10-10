# Lint oracles: lints whose premise does not hold, and fixes that change the program

Facts for findings 33–36. Found by the lint-oracle check ([`checks.md`](../checks.md), check 16:
`mirth-lab lint-check`). It runs over the standalone UI tests with a column of
allow-by-default lints turned on. For each lint that warns, it acts on the warning: it allows
the lint, removes what the lint calls unused or unreachable, rewrites code to what the lint's
premise says is equivalent, or applies the lint's machine-applicable suggestion. It then checks
that nothing else changed. Reductions are in [`tests/lint-check/`](tests/lint-check). "1.98.0"
means stable 1.98.0; "nightly" means the pinned nightly-2026-10-06.

## 33. `let_underscore_drop`: the premise and both fixes

`let_underscore_drop` (allow-by-default) warns on `let _ = <expr>;` when the value has a
destructor. Its message: "non-binding let on a type that has a destructor". It offers two
machine-applicable fixes: "consider binding to an unused variable to avoid immediately dropping
the value" (`let _unused = …`) and "consider immediately dropping the value"
(`drop(…)`).

| reduction | what happens | 1.98.0 | nightly |
|---|---|---|---|
| [`let-underscore-drop-place.rs`](tests/lint-check/let-underscore-drop-place.rs) | `let _ = x;` with `x` a local: the lint fires, but a place expression is neither moved nor dropped by `let _`. The `drop(x)` fix moves the drop from the end of `main` to that line: the program prints `drop x` before `end of main` instead of after | yes | yes |
| [`let-underscore-drop-temporary.rs`](tests/lint-check/let-underscore-drop-temporary.rs) | `let _ = A.borrow();` with `A` a `const RefCell`: the `_unused` fix keeps the `Ref` past the temporary it borrows: E0716 | yes | yes |
| [`let-underscore-drop-inference.rs`](tests/lint-check/let-underscore-drop-inference.rs) | `let _: Vec<String> = Default::default();`: the `drop(…)` fix removes the type annotation with the `let`: E0283 | yes | yes |
| [`let-underscore-drop-macro.rs`](tests/lint-check/let-underscore-drop-macro.rs) | `let _ = wrap!(String::new());` where the macro expands to `identity($x)`: the `drop(…)` fix replaces `identity($x)` in the macro definition with `drop()` and leaves the `let` alone: E0061 | yes | yes |
| [`let-underscore-drop-coroutine.rs`](tests/lint-check/let-underscore-drop-coroutine.rs) | `let _ = #[coroutine] \|\| yield 42;`: the `drop(…)` fix drops the expression's attribute: "`yield` can only be used in `#[coroutine]` closures" | — (unstable) | yes |

In the UI tests, 26 tests have a fix that does not compile or changes the output. Most fail to
infer a type once `drop(…)` replaces an annotated `let` (E0282, E0283, E0790). Others: borrowed
temporaries kept alive (E0716), `match` arms whose types the binding unified (E0308), three
coroutine tests that lose `#[coroutine]`, macros
(`lifetimes/rvalue-lifetime-drop-timing.rs`: the fix turns the macro body's
`let $pat = $expr;` into `drop($expr;`, a syntax error), and
`async-await/async-fn-send-uses-nonsend.rs`, where binding keeps a non-`Send` value alive
across an `await` ("future cannot be sent between threads safely"). In 5 run-pass tests a fix
compiles and changes the output: the drop-order tests (`destructuring-assignment/drop-order.rs`,
`drop/drop_order.rs`, `drop/issue-2735-2.rs`, `lifetimes/rvalue-lifetime-drop-timing.rs`).

`cargo fix` applies neither fix: rustfix skips a diagnostic with alternative suggestions
(#104910, open). An editor's quick fix, or a tool that applies one suggestion, uses them.
Related, about the language and not the lint: #97305 (`let _ = var` does not move `var`, closed
as intended).

## 34. Lifetime lints: fixes that change meaning

| reduction | lint | what happens | 1.98.0 | nightly |
|---|---|---|---|---|
| [`single-use-lifetimes-may-dangle.rs`](tests/lint-check/single-use-lifetimes-may-dangle.rs) | `single_use_lifetimes` | in `unsafe impl<#[may_dangle] 'a, T> Drop for Pr<'a, T>`, the fix deletes `'a, ` but not its attribute. The result `unsafe impl<#[may_dangle] T> Drop for Pr<'_, T>` compiles: the unsafe `may_dangle` promise moved from `'a` to `T` | — (unstable attribute) | yes |
| [`single-use-lifetimes-derive-hrtb.rs`](tests/lint-check/single-use-lifetimes-derive-hrtb.rs) | `single_use_lifetimes` | `x: for<'a> fn(T::SomeType<'a>)` in a `#[derive(Clone)]` struct: the fix gives `fn(T::SomeType<'_>)`: E0637 "`'_` cannot be used here" | yes | yes |
| [`unused-lifetimes-global-bound.rs`](tests/lint-check/unused-lifetimes-global-bound.rs) | `unused_lifetimes` | `where for<'a> Inherent: Clone`: the binder keeps the bound from being global, so it is not checked at the definition. The fix removes `for<'a> ` and the bound is checked: E0277 | yes | yes |

In the UI tests, `drop/dropck-eyepatch-reorder.rs` has the `may_dangle` case where the
attribute lands on a parameter that rejects it (E0199). Both lints are allow-by-default.
Earlier fixes in the same area, all closed: #117965 and #120148 (`single_use_lifetimes` fixes
that do not compile), #141758 (`unused_lifetimes` and unsafe binders).

## 35. `dead_code`: items reported unused that the program needs

`dead_code` is warn-by-default. "X is never used" invites deleting X; in these cases deleting it
(with everything else the lint reports in the same crate) breaks the program.

| reduction | what happens | 1.80.0 | 1.98.0 | nightly |
|---|---|---|---|---|
| [`dead-code-trait-in-bound.rs`](tests/lint-check/dead-code-trait-in-bound.rs) | "trait `Bar` is never used": `Bar` is in the where-clause of `impl<const N: usize> Foo<N> where [u8; N]: Bar<[(); N]>`, and `main` calls that impl's `foo` | yes | yes | yes |
| [`dead-code-trait-in-projection.rs`](tests/lint-check/dead-code-trait-in-projection.rs) | "trait `Mirror` is never used": `<A as Mirror>::Me` is in the self type of `impl<A> Foo<A, <A as Mirror>::Me>`, whose `m` `main` calls | yes | yes | yes |
| [`dead-code-defining-use.rs`](tests/lint-check/dead-code-defining-use.rs) | "function `assign` is never used" for the `#[define_opaque(Qux)]` function that is the only defining use of a used opaque type; removing it gives "unconstrained opaque type" | — | — (unstable) | yes |

The trait cases are three UI tests: `const-generics/issues/issue-69654-run-pass.rs` (which
blesses the warning: `//~ WARN trait `Bar` is never used`),
`nll/user-annotations/normalize-self-ty.rs` and `mir/issue-101844.rs`. The opaque-type case is
24 UI tests (`type-alias-impl-trait/*`, `lint/improper-ctypes/lint-73249-3.rs` and `-5.rs`, …).
Five more `dead_code` tests fail for reasons the check cannot settle (an item named through a
module path or a `decl_macro`, an `eii` attribute, an associated-const binding of the
incomplete `gca` feature); they are listed in the sweep's results, not counted here. Related
but different: #47569 (a struct used only through an associated constant, open), #110332
(below).

## 36. `trivial_numeric_casts` on an unsuffixed literal

[`trivial-numeric-cast-literal.rs`](tests/lint-check/trivial-numeric-cast-literal.rs):
`let x = 5 as i16;` warns "trivial numeric cast: `i16` as `i16`". The literal is `i16` only
because of the cast: without it, `x` is `i32` and the program prints 4 instead of 2. In
`tests/ui/packed/packed-struct-generic-layout.rs` the cast picks a generic struct's field type
(`S { …, c: 0b10000001_10000001 as i16 }`); without it, a `transmute` between the struct and
an array no longer has matching sizes (E0512). Allow-by-default lint; 1.98.0 and nightly.
Related lint issues, none about literals: #23739 (type aliases, closed), #161339 (FnDef to fn
pointer, open).

## Minor, not numbered

- `unreachable_pub` (allow-by-default): its `pub(crate)`/`pub(super)` fix does not compile when
  a `decl_macro` used elsewhere names the item (`hygiene/lexical.rs`), and in
  `imports/overwrite-different-ambig-2.rs` the narrower visibility turns the
  `ambiguous_glob_imports` warning into E0659. In
  `test-attrs/custom-test-frameworks/issue-107454.rs` it fires on a non-`pub` `#[test_case]`
  function and its fix (`pub(crate)fn`, no space) leaves the warning. 29 tests where one fix
  needs another compile when all of a lint's fixes are applied together, which is what
  `cargo fix` does; those are not counted.
- `missing_copy_implementations` (allow-by-default) on a type with a manual `ToOwned` impl: the
  `Clone` that `Copy` requires conflicts with the blanket `impl<T: Clone> ToOwned for T`
  (`autoref-autoderef/auto-deref-on-cow-regression-91489.rs`).

## Known, not counted

- `dead_code` reports inherent associated types as never used (`associated-inherent-types/*`):
  #110332, open.
- `unused_qualifications` removing a qualifier that the unqualified path needs to be
  unambiguous (`imports/ambiguous-trait-with-mixed-import-paths.rs`, E0659): #163369, open.

## What the check leaves out, and why

- An item `dead_code` reports may be used by code the lint exempts: an item with
  `#[allow(dead_code)]`, an impl of such a type, or an item named with `_`. Deleting it then
  breaks those users. That is the lint's design (it reports what only dead code uses), so the
  check deletes and accepts errors only inside such items.
- Unreachable code still takes part in type inference. Deleting an unreachable tail
  expression, or a statement that fixed a type, can change what the block infers. The check
  keeps tails and accepts inference errors (E0282–E0284).
- `missing_copy_implementations` in `staged_api` crates: the added `impl Copy` needs a
  stability attribute.
- `#163604` (deprecation and the crate's `rust-version`): `-Zhint-msrv` filters only lints
  declared with `@msrv`, and no lint at the pin declares one, so there is nothing to check yet.

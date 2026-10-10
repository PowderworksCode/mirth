# rustdoc rejects an associated-const binding rustc accepts (generic const items)

Facts for finding 35. Found by the rustdoc consistency check (`mirth-lab rustdoc-diff`) on
`generic-const-items/assoc-const-bindings.rs` (check-pass). Reduced:
[`tests/rustdoc-gca-anon-const.rs`](tests/rustdoc-gca-anon-const.rs).

## What happens

```rust
trait Owner {
    #[rustc_always_gca]
    const Q<T: ConstParamTy_>: Maybe<T>;
}
fn take2<O: Owner<Q<()> = { Maybe::Just::<()>(()) }>>(_: O) {}
```

(with the features, the impl and `enum Maybe<T>` of the test file). `rustc` (nightly-2026-10-10,
`RUSTC_BOOTSTRAP=1`) accepts it. `rustdoc` on the same file reports, at the `{ … }` of the
binding:

```
error: anonymous constants referencing generics are not yet supported
```

The constant does not mention any generic parameter; the binding is for `Q<()>`, whose type
is `Maybe<()>`.

## Where

The error is `compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs:2434`: the anonymous
constant's expected type `has_non_region_param()`. Under rustc the type is `Maybe<()>`; the
error means rustdoc's lowering of the bound reaches this check with the associated const's type
not instantiated with the binding's arguments (`Maybe<T>`). Not narrowed further.

## Versions

The test needs nightly-2026-10-10's feature names (`gca_min_const_items`); on nightly-2026-09-25
and nightly-2026-08-27 both tools reject the file for unrelated reasons (feature names,
`rustc_*` attributes). So the difference is visible from nightly-2026-10-06 on (the mirth pin)
and not earlier with this file.

## Scope

Incomplete features only (`generic_const_items`, `gca_min_const_items`,
`generic_const_parameter_types`). Low: documentation of crates using them.

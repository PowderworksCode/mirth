# rustdoc panics on an associated-const binding through a supertrait

Facts for finding 36. Found by the rustdoc consistency check (`mirth-lab rustdoc-diff`) on
`supertrait-shadowing/assoc-const.rs`, `const-generics/associated-const-bindings/supertraits.rs`
and `const-generics/associated-const-bindings/coexisting-with-type-binding.rs`, all accepted by
rustc. Reduced: [`tests/rustdoc-supertrait-assoc-const.rs`](tests/rustdoc-supertrait-assoc-const.rs).

## What happens

```rust
#![feature(gca_min_const_items)]
use std::gca;
trait B {
    #[rustc_always_gca]
    const CONST: i32;
}
trait C: B {}
fn generic<T: C<CONST = 2>>() {}
fn main() {}
```

`rustc` (nightly-2026-10-06 and nightly-2026-10-10, `RUSTC_BOOTSTRAP=1`) accepts it. `rustdoc`
panics:

```
thread 'rustc' panicked at src/librustdoc/clean/mod.rs:539:48:
called `Option::unwrap()` on a `None` value
```

The same shape with an associated type (`trait B { type T; } trait C: B {}
fn generic<X: C<T = u8>>() {}`) documents fine.

## Where

`src/librustdoc/clean/mod.rs`, `clean_assoc_item_constraint` (line 3427) looks the bound name up
among the bound trait's own items only:

```rust
                let assoc_item = cx
                    .tcx
                    .associated_items(trait_did)
                    .find_by_ident_and_kind(cx.tcx, constraint.ident, assoc_tag, trait_did)
                    .map(|item| item.def_id);
                AssocItemConstraintKind::Equality { term: clean_hir_term(assoc_item, term, cx) }
```

For `C<CONST = 2>` the constant is declared in the supertrait `B`, so `assoc_item` is `None`;
`clean_hir_term` unwraps it for a constant term (`type_of(assoc_item.unwrap())`, line 539). A
type term does not use `assoc_item`.

## Versions

The program compiles with rustc from nightly-2026-10-06 (the gca spelling); on nightly-2026-09-25
and earlier both tools reject it (E0658), and the older `associated_const_equality` feature is
removed (E0557). So it cannot be compared before the pin with this program.

Not found in the issue tracker (searched 2026-10-10).

## Scope

Incomplete features (`gca_min_const_items`, generic const arguments). An associated-const
equality bound naming a supertrait's constant through a subtrait.

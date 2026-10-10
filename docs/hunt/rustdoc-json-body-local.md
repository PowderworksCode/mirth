# rustdoc JSON: an impl in a function body names a type the output does not contain

Facts for finding 39. Found by the rustdoc consistency check (`mirth-lab rustdoc-diff`, the JSON
self-consistency part) on `lint/non-local-defs/{exhaustive,from-local-for-global,generics}.rs`
and `privacy/required-pub-in-blocks.rs`. Reduced:
[`tests/rustdoc-json-body-local.rs`](tests/rustdoc-json-body-local.rs).

## What happens

```rust
pub struct S;
pub trait Tr { type A; }
fn f() {
    pub struct Z;
    impl Tr for S { type A = Z; }
}
```

`rustdoc --crate-type lib -Zunstable-options --output-format json --document-private-items`
documents the impl (an impl applies to its types wherever it is written), so the associated
type `A` appears with

```json
"type": {"resolved_path": {"path": "Z", "id": 42, "args": null}}
```

and id 42 is in neither `index` nor `paths`: items defined inside a function body are not
documented, even with `--document-private-items`. The impl is reachable from the root (through
`S`'s impls), so rustdoc's own JSON checker, `src/tools/jsondoclint`, would report it
(`NotFound`, and "No entry in '$.paths'" for a path).

Same with nightly-2026-07-18 and 1.98.0 (format version 60) and nightly-2026-10-06 (61).

## Related

Open upstream issues report dangling ids for items rustdoc strips: a private item used in a
public signature (#113674, #119626), an item from a `#[doc(hidden)]` module (#117718, #112852).
This case differs: nothing is stripped (`--document-private-items`); the item is simply never
documented because it is local to a body. Not found in the issue tracker (searched 2026-10-10).

## Scope

Consumers of rustdoc JSON (semver checkers, documentation tools) that follow ids, on crates with
impls inside function bodies naming body-local types (which the `non_local_definitions` lint
warns about for some forms). Low.

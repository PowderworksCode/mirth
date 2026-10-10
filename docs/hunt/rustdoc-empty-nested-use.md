# rustdoc panics on an empty nested `use` group

Facts for finding 33. Found by the rustdoc consistency check (`mirth-lab rustdoc-diff`,
[`checks.md`](../checks.md) check 6) on three UI tests rustc accepts:
`imports/duplicate-empty-imports.rs`, `imports/empty-import-prefix-pass.rs`,
`imports/empty-import-prefix-pass-2015.rs`.

## What happens

```rust
use {{}};
fn main() {}
```

`rustc` compiles it (one `unused_imports` warning). `rustdoc` (nightly-2026-10-06, and today's
nightly-2026-10-10, 32dba69d6) panics:

```
thread 'rustc' panicked at src/librustdoc/clean/mod.rs:3224:36:
called `Option::unwrap()` on a `None` value
```

| input | rustc | rustdoc |
|---|---|---|
| `use {{}};` | accepts | panics |
| `use {{}, {}};` | accepts | panics |
| `mod m { use {{}}; }` | accepts | panics |
| `use {};` | accepts | ok |
| `pub use {{}};` | accepts | ok |
| `use std::{{}, {}};` | accepts | ok |
| `use {self::{}};` | accepts | ok |

The `--crate-type`, `--document-private-items` and `--output-format json` options make no
difference.

## Where

`src/librustdoc/clean/mod.rs`, `clean_use_statement_inner`, the `hir::UseKind::Nested` arm:

```rust
            for (tree, hir_id, def_id) in items {
                let mut segments = path.segments.to_vec();
                segments.extend(tree.prefix.segments.iter());
                if segments.last().unwrap().ident.name == kw::SelfLower {
```

For `use {{}}`, both the outer path and the nested tree's prefix have no segments, so
`segments` is empty.

## Versions

Bisected over nightlies: nightly-2026-09-25 (f7575a9da) does not panic, nightly-2026-09-26
(5ceaf6608) does. 1.80.0, 1.90.0 and 1.98.0 do not panic. The range includes #161349
("Unflatten `use` statements in HIR", merged 2026-09-25), which introduced the
`UseKind::Nested` arm and changed `librustdoc/clean/mod.rs`; no other PR in the range changes
`librustdoc/clean`. Not confirmed by building with it reverted.

Not found in the issue tracker (searched 2026-10-10 for rustdoc ICEs on empty and nested
imports, and for issues mentioning #161349).

## Scope

`cargo doc` on any crate, or any dependency documented with it, containing a private
`use {{}}` or `use {{}, …}` group in any module. The pattern is unusual in hand-written code
and is accepted by rustc (with `unused_imports`).

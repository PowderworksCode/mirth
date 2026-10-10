# Glob-import ambiguity depends on the order of items

Facts for finding 28. Found by the equivalent-rewrite differential (`rustc/rewrite-diff.py`, the
`reorder` rewrite: top-level items in reverse order, `use` items first) on
`tests/ui/imports/ambiguous-9.rs`; the same rewrite also flips `imports/ambiguous-14.rs`
(error → compiles) and `imports/overwrite-different-ambig-2.rs` (compiles → error).

## What happens

Two modules re-export each other's globs, and each brings its own `date_range` through a glob.
Rust's name resolution does not depend on the order in which items are written, but here the
verdict, and which function is called, do:

[`tests/glob-ambiguity-order-a.rs`](tests/glob-ambiguity-order-a.rs) (as in the UI test):

```rust
pub mod dsl {
    mod range { pub fn date_range() { println!("dsl::range") } }
    pub use self::range::*;
    use super::prelude::*;
}
pub mod prelude {
    mod t { pub fn date_range() { println!("prelude::t") } }
    pub use self::t::*;
    pub use super::dsl::*;
}
use dsl::*;
use prelude::*;
fn main() { date_range(); }
```

[`tests/glob-ambiguity-order-b.rs`](tests/glob-ambiguity-order-b.rs): the same with `mod prelude`
written before `mod dsl`.

| rustc | order A (`dsl` first) | order B (`prelude` first) |
|---|---|---|
| 1.80.0 – 1.89.0 | prints `dsl::range` | prints `dsl::range` |
| 1.90.0 – 1.93.0 | `ambiguous_glob_imports` lint error | the same lint error |
| 1.94.0 – 1.95.0 | prints `dsl::range` | E0659 `date_range` is ambiguous |
| 1.96.1 – 1.97.1 | lint error | E0659 |
| 1.98.0, nightly-2026-10-06 | E0659 | **prints `prelude::t`** |

On current compilers, one order is a hard ambiguity error and the other compiles and calls a
different function than every compiler before 1.90 did. Across releases, the order that is
accepted has swapped (1.94: A; 1.98: B).

## Expected

The same verdict for both orders. The UI test expects the ambiguity (an `ambiguous glob
re-exports` warning and an ambiguity error at the call), so B should be rejected as A is.

## Not narrowed

Which change in the glob resolution (the import resolution fixpoint, ambiguity detection for
glob re-export cycles) makes the outcome order-dependent was not investigated.

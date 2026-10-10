# rustdoc ICEs where rustc reports an error: delayed bugs whose error comes from a step rustdoc skips

Facts for finding 38. Found by the rustdoc consistency check (`mirth-lab rustdoc-diff`) on three
UI tests rustc rejects: `limits/huge-static.rs`, `type/pattern_types/nested.rs`,
`layout/gce-rigid-const-in-array-len.rs`. In each, rustc reports an ordinary error and rustdoc
reports none and ends with

```
note: no errors encountered even though delayed bugs were created
note: those delayed bugs will now be shown as internal compiler errors
error: internal compiler error: …
```

A part of the compiler both run records a delayed bug, relying on a later step to report the
real error; rustc runs that step, rustdoc does not.

## 1. A static too large for the target (stable)

```rust
static X: [u8; 1 << 61] = [0; 1 << 61];
fn main() {}
```

rustc: `error[E0080]: values of the type `[u8; 2305843009213693952]` are too big for the target
architecture` (1.80.0: "could not evaluate static initializer"). rustdoc: `error: internal
compiler error: SizeOverflow([u8; 2305843009213693952_usize])`, "delayed at
compiler/rustc_hir_analysis/src/check/check.rs:225:23". The same with `pub static`, with 1.80.0,
1.98.0 and nightly-2026-10-06. A `const` of that type, and a struct with such a field, document
fine.

`check_static_inhabited` (`check.rs:205-227`) computes the static's layout and, for any layout
error other than the foreign-static and SIMD cases, calls `span_delayed_bug` ("Generic statics
are rejected, but we still reach this case."). In rustc, evaluating the initializer then
reports E0080. rustdoc does not evaluate statics.

## 2. An invalid pattern type (internal feature)

```rust
#![feature(pattern_types, pattern_type_macro)]
use std::pat::pattern_type;
type X = pattern_type!(() is ..0);
fn main() {}
```

rustc: `error[E0277]: `()` is not a valid base type for range patterns`. rustdoc: `error:
internal compiler error: invalid base type for range pattern`, the delayed bug in
`compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs:3465`; the E0277 comes from the
well-formedness check, which rustdoc does not run on the alias. Also with
`const X: pattern_type!(() is ..0) = todo!();` and with
`pattern_type!(pattern_type!(u32 is 1..) is 0..)` (rustc: E0308).

## 3. `generic_const_exprs` (incomplete feature)

`layout/gce-rigid-const-in-array-len.rs`: rustdoc ICEs with "Missing value for constant, but no
error reported?". Not reduced.

## Scope

Documenting code that does not compile: no `cargo doc` of a crate that builds is affected, but
rustdoc panics instead of reporting the error. Case 1 is stable Rust since at least 1.80.0.
Not found in the issue tracker (searched 2026-10-10 for rustdoc with SizeOverflow, too-large
statics, and the pattern-type message).

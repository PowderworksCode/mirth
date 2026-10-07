# Incremental rebuilds encode a string literal twice in metadata where a clean build encodes it once

<!-- Draft issue for rust-lang/rust. Regression from #116707 (1.90). -->

After an incremental rebuild, a crate's `.rmeta` can contain two copies of the same
string-literal allocation where a clean build of the same source has one. The copy decoded
from the incremental cache gets its own `AllocId` instead of being deduplicated with the one
built afresh.

### Reproduction

```rust
// before.rs
#[inline] pub fn a() -> &'static str { "literal" }
#[inline] pub fn b() -> &'static str { "literal" }
```

```rust
// after.rs: only b's body changes
#[inline] pub fn a() -> &'static str { "literal" }
#[inline] pub fn b() -> &'static str { let s = "literal"; s }
```

```sh
mkdir rebuilt clean
cp before.rs lib.rs
rustc --edition 2024 --crate-type lib --emit=metadata,link -C incremental=incr --out-dir rebuilt lib.rs
cp after.rs lib.rs
rustc --edition 2024 --crate-type lib --emit=metadata,link -C incremental=incr  --out-dir rebuilt lib.rs
rustc --edition 2024 --crate-type lib --emit=metadata,link -C incremental=clean --out-dir clean   lib.rs
cmp rebuilt/liblib.rmeta clean/liblib.rmeta     # differ
grep -c -a -o literal rebuilt/liblib.rmeta      # 2
grep -c -a -o literal clean/liblib.rmeta        # 1
```

I expected the two `.rmeta` files to be identical. Instead the rebuilt one is 16 bytes longer:
it encodes the allocation for `"literal"` twice. (`--emit=link` is needed only because
MIR is encoded only when the crate is also code-generated, as in `cargo build`.)

### Root cause

String literals are allocated with `TyCtxt::allocate_bytes_dedup(bytes, CTFE_ALLOC_SALT)`
(`rustc_mir_build/src/builder/expr/as_constant.rs`), so in one session every `"literal"`
in MIR shares one `AllocId`. The metadata encoder collects allocations by `AllocId`
(`EncodeContext::interpret_allocs`, an `FxIndexSet<AllocId>`), so a clean build encodes it
once.

Allocations decoded from the incremental cache, or from metadata, are not deduplicated.
`AllocDecodingSession::decode_alloc_id` (`rustc_middle/src/mir/interpret/mod.rs`) handles
`AllocDiscriminant::Alloc` with `reserve_and_set_memory_alloc`, which always reserves a fresh
`AllocId`. Its neighbours `Fn` and `VTable` go through the deduplicating
`reserve_and_set_fn_alloc` and `reserve_and_set_vtable_alloc` with `CTFE_ALLOC_SALT`.

In the rebuild, `b` changed, so `optimized_mir(b)` is computed again and its literal gets the
deduplicated `AllocId`. `a` did not change, so `optimized_mir(a)` is decoded from the cache,
and its literal gets a fresh one. The metadata then has two allocations with identical bytes.

It started with #116707 ("Create an `AllocId` for `ConstValue::Slice`", merged
2025-07-24, in 1.90): `nightly-2025-07-24` (`ace633090`) is not affected, and
`nightly-2025-07-26` (`430d6eddf`) is. Before that change a slice constant carried its
bytes inline, so each use was encoded separately, the same way in both builds: 1.89.0
has three copies of `"literal"` in both. From 1.90.0 to the current nightly, the rebuild has
two and the clean build one.

### Impact

- An incremental rebuild's `.rmeta`, and the crate hash computed from it (#154724), depend on
  which MIR bodies came from the cache. So does the metadata of crates that depend on it.
- It does not accumulate: alternating the two versions of `b` for six sessions keeps the
  rebuilt metadata at two copies.
- I found no effect on generated code: a dependent comparing `a().as_ptr() == b().as_ptr()`
  prints `true` either way, at `-C opt-level=0` and `3`, because codegen merges identical
  constant data anyway.
- It reaches real builds through derives: `#[derive(Debug)]`'s `fmt` is `#[inline]` and
  contains the type's name as a literal. Any other literal with the same text, such as a
  `const NAME: &str = "Drawing"` produced by another derive, is encoded once in a clean build
  and twice after an incremental rebuild that recompiled only one of them. That is how I first
  saw it.

### Suggested fix

Decode immutable memory allocations the way they were created, deduplicated:

```diff
             AllocDiscriminant::Alloc => {
                 let alloc = <ConstAllocation<'tcx> as Decodable<_>>::decode(decoder);
-                decoder.interner().reserve_and_set_memory_alloc(alloc)
+                if alloc.inner().mutability.is_not() {
+                    decoder.interner().reserve_and_set_memory_dedup(alloc, CTFE_ALLOC_SALT)
+                } else {
+                    decoder.interner().reserve_and_set_memory_alloc(alloc)
+                }
             }
```

With it, together with the fix for #… (the other issue), the reproduction and all ten
single-threaded incremental edits to a larger test fixture give identical metadata. These all
still pass: `tests/incremental` (180), `tests/ui/{consts,statics,const-generics}` (1844),
`tests/codegen-llvm` (1122), and the 532 UI and 46 run-make tests about metadata and crate
loading that I run. The full test suite was not run.

It does more than strictly needed: it would also merge an immutable allocation decoded from a
dependency's metadata with an identical local one, and two immutable allocations that were
distinct when created (results of different constants, say). As far as I know, neither has
a guaranteed unique address, but someone who knows the const-eval memory model should
confirm. A narrower fix records, when encoding, whether an allocation was created
through deduplication and with which salt, and repeats exactly that when decoding.

A regression test in the style of `tests/run-make`, which fails before the change and passes
after, is attached (`incr-metadata-literal-dedup/rmake.rs`).

### How it was found

[mirth](https://github.com/PowderworksCode/mirth) checks properties of rustc's metadata
handling across Cargo builds. One property is that an incremental rebuild after an edit
encodes the same metadata as a clean build of the edited source. Comparing what the two
compiler processes did, the rebuild decoded cached MIR (resolving foreign expansions and
source files to do so), which led to the cached allocations.

### Meta

`rustc --version --verbose`:
```
rustc 1.101.0-nightly (ea137335b 2026-10-05)
binary: rustc
commit-hash: ea137335b78829b4514bf1b4c16302f74fab8581
host: x86_64-unknown-linux-gnu
```

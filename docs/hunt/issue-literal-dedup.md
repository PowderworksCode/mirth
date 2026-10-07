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

Decode an allocation the way it was created. When encoding a memory allocation, check
whether the deduplication map holds exactly this `AllocId` for it, under `CTFE_ALLOC_SALT`.
If it does, encode it with a new discriminant, and decode that one through
`reserve_and_set_memory_dedup` (attached, `alloc-dedup-on-decode.patch`):

```diff
         GlobalAlloc::Memory(alloc) => {
-            AllocDiscriminant::Alloc.encode(encoder);
+            let deduplicated = tcx.alloc_map.dedup.lock().get(&(GlobalAlloc::Memory(alloc), CTFE_ALLOC_SALT))
+                == Some(&alloc_id);
+            if deduplicated {
+                AllocDiscriminant::DedupAlloc.encode(encoder);
+            } else {
+                AllocDiscriminant::Alloc.encode(encoder);
+            }
             alloc.encode(encoder);
         }
 ...
+            AllocDiscriminant::DedupAlloc => {
+                let alloc = <ConstAllocation<'tcx> as Decodable<_>>::decode(decoder);
+                decoder.interner().reserve_and_set_memory_dedup(alloc, CTFE_ALLOC_SALT)
+            }
```

A simpler change, deduplicating every immutable allocation on decode, is wrong. I tried it
first, and it fixes this reproduction but breaks the same property elsewhere. It merges
allocations that a clean build keeps apart, because they were never deduplicated when
created. On serde at commit `2f58a20` ("Inline is_human_readable", 2017), an incremental
rebuild then encoded 59 bytes fewer of `interpret-alloc-index` than a clean build
(`-Zmeta-stats`). With the narrower change above, that commit and its neighbours match.

Two things for a reviewer. Only `CTFE_ALLOC_SALT` is handled: allocations deduplicated
under other salts (Miri's) are encoded as before, which only matters if those reach an
incremental cache. And the change adds a discriminant to the encoding of allocations in
metadata and in the incremental cache, so it may want a metadata version bump.

With all three changes proposed in this series applied (this one and the two in #… and
#…), each attached regression test passes, and each fails when only its own change is
removed. These rustc tests still pass: `tests/incremental` (180), the UI tests in
`tests/ui/{deprecation,crate-loading,rmeta,extern,cross-crate}` (532), 46 metadata-related
`tests/run-make` tests, `tests/ui/{consts,statics,const-generics}` (1844) and
`tests/codegen-llvm` (1122). The full test suite was not run. A fuzzer making random edits to a 1,200-line test workspace ran 10,717 edits on the patched compiler, 8,936 of which built and were compared with a clean build, and a replay of ten crates' git histories compared 4,862 commits; neither found a difference.

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

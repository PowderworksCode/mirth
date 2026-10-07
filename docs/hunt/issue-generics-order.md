# Incremental rebuilds encode `generics_of` with `param_def_id_to_index` in a different order from clean builds

<!-- Draft issue for rust-lang/rust. Related: #163878 (same field, parallel front end). -->

An incremental rebuild produces different `.rmeta` bytes from a clean build of the same
source, single-threaded. The cause is that `Generics::param_def_id_to_index`, an
`FxHashMap`, does not survive a round trip through the incremental cache in the same order.

### Reproduction

```rust
// lib.rs
pub struct Grid<A, B, C>(A, B, C);
impl<A, B, C> Grid<A, B, C> { pub const AREA: usize = 1; }
```

```sh
rustc --edition 2024 --crate-type lib --emit=metadata -C incremental=incr lib.rs -o first.rmeta
sed -i '1i // a comment' lib.rs
rustc --edition 2024 --crate-type lib --emit=metadata -C incremental=incr  lib.rs -o rebuilt.rmeta
rustc --edition 2024 --crate-type lib --emit=metadata -C incremental=clean lib.rs -o clean.rmeta
cmp rebuilt.rmeta clean.rmeta    # differ
```

I expected `rebuilt.rmeta` and `clean.rmeta` to be identical: the same source, the same
compiler, the same flags. Instead they differ in 21 bytes: the 16-byte hash in the header
and the order of three `(DefIndex, u32)` pairs.

It is not specific to this example. A comment added at the top of `lib.rs` gives an
incremental rebuild different metadata from a clean build for
[`either`](https://crates.io/crates/either) 1.13.0, `smallvec` 1.13.2, `memchr` 2.7.4 and
`arrayvec` 0.7.6. With the change suggested below, all four match.

### Root cause

`generics_of` is `cache_on_disk`. `Generics::param_def_id_to_index` is an
`FxHashMap<DefId, u32>`, and the `Encodable` impl for `HashMap` writes entries in iteration
order. `Decodable` collects them back in that order:

- In the first session the map is built by
  `own_params.iter().map(|param| (param.def_id, param.index)).collect()`, inserting in
  parameter order, then written to the incremental cache in its iteration order.
- In the next session `generics_of` is green and decoded from the cache, so the map is
  rebuilt by inserting in the first map's iteration order.

Iteration order of a hashbrown table is bucket order, and which bucket a key lands in
depends on insertion order when keys collide. In the example all three keys hash to
the same home bucket of a 4-bucket table (FxHash of `DefId`s 12, 13 and 14, as
`(krate << 32) | index`). The first key inserted takes bucket 3 and the others wrap to
buckets 0 and 1, so iteration order is the insertion order rotated by one. Decoding inserts
in that rotated order and rotates it again. The metadata encoder then writes the decoded
map in its own iteration order:

| session | encoded order (`DefIndex` → index) |
|---|---|
| clean build | 13→1, 14→2, 12→0 |
| incremental rebuild 1 | 14→2, 12→0, 13→1 |
| incremental rebuild 2 | 12→0, 13→1, 14→2 |
| incremental rebuild 3 | 13→1, 14→2, 12→0 (matches the clean build again) |

The table is from the `.rmeta` files rustc wrote (from byte 1355, after `03`
for the length; each entry is `CrateNum`, `DefIndex`, index). A standalone program using `std::collections::HashMap` with
`rustc_hash::FxBuildHasher` 2.1.1, building and round-tripping the map the same way, prints
exactly the same sequence (attached as `hashmap-roundtrip`; `cargo run -- 12 13 14`).

That is also why the reproduction is sensitive to unrelated items: adding or removing
items changes the `DefIndex`es, and so whether keys collide. Two colliding keys swap on
every round trip; keys that don't collide are stable.

### Impact

- An incremental rebuild's `.rmeta`, and the crate hash computed from it (#154724), depend
  on how many times each `generics_of` result has been round-tripped through the cache, not
  only on the source. So does the `.rlib` that embeds it, and the metadata of every crate
  that depends on it, since those record the crate hash.
- I found no effect on generated code or diagnostics: the map is only used for lookups.
- It hides other incremental bugs from anyone comparing incremental and clean outputs. I
  found it while doing that, and it masked a second issue (#…).

The instability was known in one place: `impl Debug for Generics` collects and sorts the map
before printing it, under `#[expect(rustc::potential_query_instability)]` and the comment
"ironically, we get this warning because of what we're trying to fix". The encoding was
not given the same treatment.

### Suggested fix

Keep the map's order deterministic across encoding and decoding. Making the field an
`FxIndexMap<DefId, u32>` does that: an `IndexMap` iterates in insertion order, and decoding
inserts in the encoded order, so a round trip is the identity. It is a small change
(`compiler/rustc_middle/src/ty/generics.rs`; the `#[expect]` in the `Debug` impl then has
nothing to expect and goes too), and every construction site uses `.collect()`
and every use is `get` or indexing. Another option is not to encode the map at all and
rebuild it from `own_params` when decoding.

With the `FxIndexMap` change, the reproduction, the four crates above, and nine of ten edits
to a larger test fixture give identical metadata after an incremental rebuild. (The tenth is
the issue about string literals, #….)

With all three changes proposed in this series applied (this one and the two in #… and
#…), each attached regression test passes, and each fails when only its own change is
removed. These rustc tests still pass: `tests/incremental` (180), the UI tests in
`tests/ui/{deprecation,crate-loading,rmeta,extern,cross-crate}` (532), 46 metadata-related
`tests/run-make` tests, `tests/ui/{consts,statics,const-generics}` (1844) and
`tests/codegen-llvm` (1122). The full test suite was not run. A fuzzer making random edits to a 1,200-line test workspace ran 10,717 edits on the patched compiler, 8,936 of which built and were compared with a clean build, and a replay of ten crates' git histories compared 4,862 commits; neither found a difference. The `Generics`
struct is the only one I found that derives `TyEncodable`/`Encodable`, reaches metadata and
has a `HashMap` field; the others with such fields (`TypeckResults::used_trait_imports`,
`CrateInfo`, the on-disk cache footer) do not reach `.rmeta`.

#163878 reports the same field as a source of non-reproducibility under `-Zthreads`. The
change here would fix the single-threaded case. Whether it also fixes the parallel one depends
on whether those builds differ only in this map's order. The suggested change was not tested
against #163878's reproduction.

A regression test in the style of `tests/run-make` is attached
(`incr-metadata-generics-order/rmake.rs`). It fails on the current nightly and passes with
the change. It tries the example after
0 to 7 unrelated items, since collisions depend on the `DefIndex`es.

### How it was found

[mirth](https://github.com/PowderworksCode/mirth) checks properties of rustc's metadata
handling across Cargo builds. One property is that an incremental rebuild after an edit
encodes the same metadata as a clean build of the edited source.

### Meta

`rustc --version --verbose`:
```
rustc 1.101.0-nightly (ea137335b 2026-10-05)
binary: rustc
commit-hash: ea137335b78829b4514bf1b4c16302f74fab8581
host: x86_64-unknown-linux-gnu
```

The example also reproduces with 1.95.0 and 1.98.1, where only the 6 bytes of the
reordered pairs differ (those releases do not yet derive the header hash from the
metadata). Releases 1.78 to 1.82 show much larger
differences between incremental and clean metadata for this example, from some other cause,
so I could not tell when this one started.

# With `-Zmir-opt-level=3`, an incremental rebuild encodes an allocation from inlined `core` MIR twice where a clean build encodes it once

<!-- Draft issue for rust-lang/rust. Seen on 1.60.0 through nightly-2026-10-06. Related to the
string-literal case in hunt/issue-literal-dedup.md: an allocation shared in a clean session is
not shared after a round trip through the incremental cache. -->

An incremental rebuild produces different `.rmeta` from a clean build of the same source: the
rebuild's metadata has one more entry in its allocation table.

### Reproduction

```sh
echo 'pub fn f<T>(v: &[T]) -> Option<&[T]> { v.get(..3) }' > lib.rs
F="--edition 2021 --crate-type lib --crate-name x --emit=metadata,link -Zmir-opt-level=3"
rustc $F -C incremental=incr  --out-dir rebuilt lib.rs
echo 'pub fn g<T>(v: &[T]) -> Option<&[T]> { v.get(..3) }' >> lib.rs
rustc $F -C incremental=incr  --out-dir rebuilt lib.rs
rustc $F -C incremental=clean --out-dir clean   lib.rs
cmp rebuilt/libx.rmeta clean/libx.rmeta   # differ
```

I expected the two to be identical. With `-Zmeta-stats`, the whole difference is in
`interpret-alloc-index`: the rebuild's metadata has an extra allocation. Two clean builds agree.

| variation | result |
|---|---|
| as above, on `nightly-2026-10-06` | differs |
| `-Zmir-opt-level=2`, or `-Copt-level=3` alone | same |
| `-Zmir-opt-level=3 -Zinline-mir=no` | same |
| the same result from a generic local `#[inline]` helper instead of `get` (`if v.len() >= 3 { Some(&v[..3]) } else { None }`) | same |
| non-generic (`v: &[u8]`) | same |

An earlier form of the reproduction (`v.get(..N)?.try_into().ok()` with a const parameter and
`-Cdebuginfo=2`) also differs on 1.60.0, 1.65.0, 1.70.0, 1.75.0, 1.80.0, 1.85.0 and 1.90.0
(with `RUSTC_BOOTSTRAP=1`).

### What differs

At this level `f`'s optimized MIR returns a constant held in an allocation, because `T` is
generic and the value cannot be a scalar:

```text
_0 = const Indirect { alloc_id: alloc1, offset: Size(0 bytes) }: Option<&[T]>;
```

In a clean session `f` and `g` refer to the same allocation, so the metadata encodes it once.
In the rebuild, `f` is green and its optimized MIR is decoded from the incremental cache,
where the allocation was encoded as plain memory; decoding gives it a new `AllocId`, while
`g`, computed fresh, refers to the shared one. Metadata then encodes both.

The sharing comes from inlining: the constant arrives with the MIR of
`<[T]>::get` and its `SliceIndex` impl, inlined from `core`. Allocations decoded from a crate's
metadata are decoded once per session and shared by every function that inlines that MIR, but
the incremental cache stores a copy rather than a reference to `core`'s allocation. This fits
every variation above: no inlining, no difference; a generic local helper (whose MIR is not
decoded from another crate), no difference; a non-generic function (where the value is a
scalar), no difference.

A change to the incremental cache confirms it
([`upstream-alloc-reference.patch`](upstream-alloc-reference.patch), experimental): when an
allocation that was decoded from another crate's metadata is written to the cache, it is
written as that crate's stable id and the allocation's index there, with its contents, and
decoding it gives the same `AllocId` as decoding that crate's allocation (if the contents
agree). With it, both reproductions above build the same incrementally as clean.

It does not fix everything the fuzzer found: one of its cases on the test workspace, at
`-Zmir-opt-level=4`, still has an extra allocation with the change, so there is at least one
more source (perhaps MIR inlined from a function of the same crate, whose own MIR came from
the cache). With the change, the reuse check also reports a string constant whose cached and
fresh encodings differ, which may be the change's own doing. Not investigated further.

The string-literal case ([report](issue-literal-dedup.md)) is the same kind of loss: an
allocation shared in a clean session is not shared again after a round trip through the cache.
A fix along the same lines would encode, in the incremental cache, an allocation that came
from another crate's metadata as a reference to it rather than as a copy.

### How it was found

[mirth](https://github.com/PowderworksCode/mirth)'s fuzzer, adding `-Zmir-opt-level=4` to every
build, reported incremental rebuilds whose metadata differed from clean builds after an edit
that duplicated this function in its test workspace. A patch that checks reused results inside
rustc ([`verify-reuse.patch`](verify-reuse.patch)) named the two MIR bodies and the allocation:

```text
rustc-verify-reuse: allocation shared differently: query `optimized_mir` for DefId(0:125 ~ sink_core[7fc5]::consts::first_n_fuzz19), computed this session uses alloc43 where a fresh computation uses alloc43, but query `optimized_mir` for DefId(0:122 ~ sink_core[7fc5]::consts::first_n), green uses alloc42 for it
  allocation: memory, 16 bytes, align 8, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
```

The test workspace was then reduced automatically, and by hand to the one line above.

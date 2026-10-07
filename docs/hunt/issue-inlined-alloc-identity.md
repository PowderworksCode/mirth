# With `-Zmir-opt-level=3` and debuginfo, an incremental rebuild encodes an allocation twice where a clean build encodes it once

<!-- Draft issue for rust-lang/rust. Seen on 1.60.0 through nightly-2026-10-06. Related to the
string-literal case in hunt/issue-literal-dedup.md: an allocation shared in a clean session is
not shared after a round trip through the incremental cache. -->

An incremental rebuild produces different `.rmeta` from a clean build of the same source: the
rebuild's metadata has one more entry in its allocation table.

### Reproduction

```sh
cat > lib.rs <<'EOF'
pub fn first_n<const N: usize>(v: &[u8]) -> Option<[u8; N]> {
    v.get(..N)?.try_into().ok()
}
EOF
F="--edition 2021 --crate-type lib --crate-name x --emit=metadata,link -Zmir-opt-level=3 -Cdebuginfo=2"
rustc $F -C incremental=incr  --out-dir rebuilt lib.rs
cat >> lib.rs <<'EOF'
pub fn first_m<const N: usize>(v: &[u8]) -> Option<[u8; N]> {
    v.get(..N)?.try_into().ok()
}
EOF
rustc $F -C incremental=incr  --out-dir rebuilt lib.rs
rustc $F -C incremental=clean --out-dir clean   lib.rs
cmp rebuilt/libx.rmeta clean/libx.rmeta   # differ
```

I expected the two to be identical. With `-Zmeta-stats`, the whole difference is in
`interpret-alloc-index` (18 bytes on a larger crate). Two clean builds agree; without
`-Cdebuginfo=2`, or at `-Zmir-opt-level=2` and below (so also at `-Copt-level=3`), the rebuild
agrees with the clean build. It reproduces on 1.60.0, 1.65.0, 1.70.0, 1.75.0, 1.80.0, 1.85.0,
1.90.0 and `nightly-2026-10-06` (with `RUSTC_BOOTSTRAP=1` on stable).

### What differs

The optimized MIR of `first_n` at this level has a constant `Option::<&[u8]>::None` held in a
16-byte allocation (`_4 = const Option::<&[u8]>::None;` with `--emit=mir`). In a clean session
`first_n` and `first_m` refer to the same allocation, so the metadata encodes it once. In the
rebuild, `first_n` is green and its optimized MIR is decoded from the incremental cache, where
its allocations were encoded as plain memory; decoding gives it a new `AllocId`, while
`first_m`, computed fresh, refers to the shared one. Metadata then encodes both.

Where the shared allocation comes from I have not confirmed. Interning a constant for
propagation does not deduplicate (`intern_with_temp_alloc`), so the sharing more likely comes
from MIR inlined from `core` (here `<[u8]>::get`): allocations decoded from a crate's metadata
are decoded once per session and shared by every function that inlines that MIR, but the
incremental cache stores a copy, not a reference to the upstream crate's allocation. That would
also explain why debuginfo matters (the constant appears in inlined debuginfo) and why only
`-Zmir-opt-level=3` and above (more inlining).

The string-literal case ([report](issue-literal-dedup.md)) is the same kind of loss: an
allocation shared by deduplication is not shared again after a round trip through the cache.

### How it was found

[mirth](https://github.com/PowderworksCode/mirth)'s fuzzer, adding `-Zmir-opt-level=4` to every
build, reported incremental rebuilds whose metadata differed from clean builds after an edit
that duplicated this function in its test workspace. A patch that checks reused results inside
rustc ([`verify-reuse.patch`](verify-reuse.patch)) named the two MIR bodies and the allocation:

```text
rustc-verify-reuse: allocation shared differently: query `optimized_mir` for DefId(0:125 ~ sink_core[7fc5]::consts::first_n_fuzz19), computed this session uses alloc43 where a fresh computation uses alloc43, but query `optimized_mir` for DefId(0:122 ~ sink_core[7fc5]::consts::first_n), green uses alloc42 for it
  allocation: memory, 16 bytes, align 8, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
```

The test workspace was then reduced automatically to the three lines above.

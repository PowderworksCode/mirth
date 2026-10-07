# Checking reuse inside rustc

**The idea.** When an incremental session reuses something from the cache, rustc could
also compute it afresh and compare the two, behind a `-Z` flag, or always in debug builds of
the compiler. Every CI job and every user who turns the flag on would then run P6 on their
own code, at the moment of reuse, instead of only in mirth's builds.

## Is anyone doing it?

Not that we could find (October 2026):

- **`-Zincremental-verify-ich`** is the nearest thing. When a query result is loaded from
  the incremental cache, it hashes the loaded value again and compares it with the hash the
  previous session stored. Without the flag it checks a rotating 1-in-32 subset
  (`should_verify_loaded_value`, `rustc_query_impl/src/incremental.rs`). It never recomputes
  a result, so it cannot see a value that was wrong when stored. It ignores fields marked
  `#[stable_hash(ignore)]`: `Generics::param_def_id_to_index`, the field behind the first
  bug in [`hunt/`](hunt), is one. And it never looks at work products: reused metadata and
  object files are not query results. The fuzzer ran every build with it, and it reported
  none of the three bugs.
- **`#[rustc_clean]`** and `-Zquery-dep-graph` assert in `tests/incremental` which nodes
  are reused or recomputed, for hand-written cases.
- **The 2026 project goal ["Incremental Systems Rethought"](https://goals.rust-lang.org/2026/incremental-system-rethought.html)**
  plans *more* reuse: `cargo build` reusing `cargo check`'s work, and data dependencies and
  diffing. Its plan does not mention verifying reused results. More reuse is more places for
  stale reuse, so a check like this would back it up.
- Searches of rust-lang/rust issues and PRs for verifying reused work products, recomputing
  green queries, or shadow verification found nothing relevant. The one open issue nearby
  is #162601, an ICE when an LTO work product is missing.

## What it would check

Each kind of reuse has a natural comparison:

| reused | how it is reused today | the shadow check |
|---|---|---|
| a query result marked green | loaded from the cache, or kept without loading | recompute it (force the query as if red) and compare with the loaded value, by value, not by stable hash |
| metadata | the saved `.rmeta` hard-linked or copied when its node is green (`encode_metadata`) | encode it again and compare bytes; this would have caught the stale-metadata bug the first time it happened |
| an object file (codegen unit) | the saved `.o` reused when its CGU is green | codegen it again and compare, or compare the LLVM IR |
| diagnostics | replayed from the cache | compare with the diagnostics the recomputation emits |

Recomputing everything doubles a build's cost, so the flag would usually sample, as
`-Zincremental-verify-ich` does: a deterministic subset per session, rotating so that every
reused item is checked over many sessions, with an option to check everything.

## The patch

[`hunt/verify-reuse.patch`](hunt/verify-reuse.patch), against the pinned rustc, does this for
metadata, query results and codegen units. It is on when `RUSTC_VERIFY_REUSE` is set (`verbose` also
counts what was checked), and every fuzzer and replay build now runs with it: a line
starting `rustc-verify-reuse:` is a finding of kind `verify-reuse` (`reuse` in the replay).

**Metadata.** When `encode_metadata` reuses the saved `.rmeta`, it also encodes the metadata
again, into a file next to the output, and compares the bytes. A difference prints the
first differing byte and keeps the fresh file as `<output>.rmeta.fresh`.

**Query results.** At the end of the session, just before the dependency graph and the cache
are saved, every value of a query cached on disk whose node is green is computed again by
its provider, outside dependency tracking, and compared with the value in use three ways:

- the stable hash;
- the `Debug` text, which sees fields the stable hash ignores, with on-demand caches
  (`OnceLock`) blanked, `UnordMap`/`UnordSet` elements sorted and `AllocId` numbers removed;
- the bytes the cache would encode for the value alone, allocations included, which see the
  order of hash maps (not compared for values containing an `UnordMap` or `UnordSet`, which
  encode in an order nothing may observe).

And across values: for queries whose values refer to allocations (MIR, const evaluation),
each allocation a fresh computation refers to is mapped to the one the value in use refers
to. One fresh allocation mapped to two different ones means the values in use do not share an
allocation that a clean session would share.

The first version recomputed a value as it was loaded. That runs the provider while the query
that asked for the value is still executing, and it cycled (`E0391`, "cycle detected when
finding item bounds"). At the end of the session nothing is executing, and the previous
session's cache can still be read.

Not recomputed:

- queries whose provider reads MIR or THIR that has already been stolen
  (`optimized_mir`, `mir_for_ctfe` and others, for a definition whose bodies were built this
  session; the same definitions are checked when their bodies were not built);
- `mir_borrowck`, which reads the MIR of nested bodies too, and
  `coroutine_by_move_body_def_id`, which makes a definition;
- values of feedable queries for definitions the compiler made up (the associated type of
  an `impl Trait` in a trait, an elided lifetime added by lowering, the type of a const
  argument), which are set rather than computed;
- anything named in `RUSTC_VERIFY_REUSE_SKIP` (comma-separated query names).

**Codegen units.** When a codegen unit is generated, its unoptimized LLVM IR is kept in the
crate's incremental directory (`verify-reuse/<unit>.ll`, outside any one session's
directory). When a later session reuses the unit's object code, the unit is generated again,
outside dependency tracking, and its IR compared with the kept one; a difference keeps the
fresh IR as `<unit>.fresh.ll`. Inline assembly's `srcloc` cookies are left out of both:
they are raw byte positions that an edit earlier in the source map moves, and rustc emits
them only where a reused module never goes through LLVM again. Both sessions need the check
on. This is the check that would have caught [finding 6](hunt.md) on the spot: with
`-Zembed-source`, its reproduction prints

```text
rustc-verify-reuse: codegen unit `21p0vejx42dvnj8u08b23lumy` of `lib` reused from the incremental cache differs from a fresh codegen (unoptimized code, 2102 and 2135 bytes)
```

and on `fixtures/sink` an edit and rebuild checks 257 reused units and prints nothing (with
finding 6's checksum left out of incremental sessions by
[`hunt/debuginfo-checksum-stopgap.patch`](hunt/debuginfo-checksum-stopgap.patch), a testing aid,
not a fix). It costs more than the rest: about 40% on that rebuild.

Replayed diagnostics are not checked yet, and diagnostics LLVM emits while compiling a unit
are not replayed at all ([finding 7](hunt.md)).

## Does it find the known bugs?

Each fix reverted in turn on the patched compiler, with the reproduction from
[`hunt/`](hunt) and an edit to `fixtures/sink`:

| bug | fix reverted | the check prints |
|---|---|---|
| `param_def_id_to_index` order ([report](hunt/issue-generics-order.md)) | `generics-index-map.patch` | ``query `generics_of` for DefId(0:11 ~ lib[ab28]::{impl#0}), green, differs from a fresh computation (encoding)`` |
| literal allocation deduplication ([report](hunt/issue-literal-dedup.md)) | `alloc-dedup-on-decode.patch` | ``allocation shared differently: query `optimized_mir` for DefId(0:4 ~ lib[ab28]::b), computed this session uses alloc1 where a fresh computation uses alloc1, but query `optimized_mir` for DefId(0:3 ~ lib[ab28]::a), green uses alloc2 for it`` |
| stale metadata reuse ([report](hunt/issue-stale-metadata-reuse.md)) | `metadata-source-files.patch` | ``metadata of `sink_core` reused from the incremental cache differs from a fresh encoding (237382 and 237383 bytes, first difference at byte 8)`` |

With all three fixes, the same builds print nothing. Neither of the first two is visible to
the stable hash, so `-Zincremental-verify-ich` cannot see them.

**Cost.** An incremental rebuild of `fixtures/sink` after an edit recomputes about 11,800 green
values and takes 2.65 s instead of 2.33 s.

**Noise removed on the way.** Before the `Debug` text and encoding comparisons were
normalized, they reported values that were equal: lazily filled caches in MIR bodies, the
iteration order of `UnordMap`s in `typeck_root` and `specialization_graph_of`, and the
numbers of allocations in const-evaluation results.

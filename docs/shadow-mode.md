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

## Where it would go first

Metadata is the cheapest to start with and has the most recent bug: the reuse decision is
one function (`encode_metadata` in `rustc_metadata/src/rmeta/encoder.rs`), and encoding
again into a temporary file and comparing bytes is a small change. A failure would name
the first differing byte, which `-Zmeta-stats`'s sections place in a table.

Not started. This page is the record of what was looked at.

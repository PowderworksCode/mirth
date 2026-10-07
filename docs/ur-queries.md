# Bug patterns as Ur queries

Each bug mirth found came from a pattern in rustc's source, not a one-off mistake. Written
as a query, a pattern can be run over the whole compiler to find the other places it
occurs. [`ur/rustc/RoundTrip.rsc`](../ur/rustc/RoundTrip.rsc) holds one query per bug,
written as classifiers for [Ur](https://github.com/PowderworksCode/codebase/tree/main/projects/ur)'s
`ur rewrite --classify`. Each is made as general as it can be while still finding its bug.

```sh
ur rewrite --classify --rules ur/rustc/RoundTrip.rsc --report report.json path/to/rust/compiler
```

On rustc `ea137335b`'s `compiler/` (2,215 files) the queries run in 0.9 seconds with
Ur `ba84ca7`.

## The queries and what they found

| query | the pattern | its bug | sites | after triage |
|---|---|---|---:|---|
| `hashOrderEncoded` | a field whose type is a hash-ordered collection (`FxHashMap`, `FxHashSet`, `HashMap`, `HashSet`, `UnordMap`, `UnordSet`) in a type that derives an encoder | `Generics::param_def_id_to_index` ([report](hunt/issue-generics-order.md)) | 12 | the bug; the rest never reach `.rmeta` |
| `hashOrderAlias` | a type alias for such a collection, which anything encoding a value of it writes in iteration order | #159677 (`DocLinkResMap`), from before its fix | 14 | none reach an encoder today |
| `decodedFresh` | a decoder calling a function that reserves a fresh identity (`reserve*`, `fresh*`, `*next_id*`) where neither its name nor its definition deduplicates | the string literal decoded twice ([report](hunt/issue-literal-dedup.md)) | 1 | the bug |
| `untrackedWhileEncoding` | inside an encoder (an `Encode*` impl or a function named `encode*`), a read of the session, the environment or the clock | stale metadata from the untracked source map ([report](hunt/issue-stale-metadata-reuse.md)) | 23 | the bug, and one more read of the same data; the rest benign |

**Each query finds the bug it came from.** `hashOrderAlias` also finds #159677, a bug fixed
in July 2026: with `DocLinkResMap` put back to the `UnordMap` it was before #159718, the
query flags it. It was written for bug 1 and not tuned to that one.

**Triage.** No new bug turned up. Every other site was checked by hand:

- `hashOrderEncoded`: `UnordMap.inner` and `UnordSet.inner` show that every `UnordMap` and
  `UnordSet` is encoded in iteration order, so any reaching metadata would be this bug
  again. The hash-ordered query results that metadata reads are encoded sorted
  (`stability_implications` uses `to_sorted_stable_ord`), and `doc_link_resolutions` is an
  `FxIndexMap` since #159718. The other fields are in the incremental cache's footer,
  `TypeckResults`, `WorkProduct` and codegen's `CrateInfo`, which never reach `.rmeta`; the
  cache does not have to be reproducible. `FormatArguments.names` is in the AST, which
  metadata does not encode.
- `hashOrderAlias`: fed back into the field query, no alias is a field of an encoded type;
  `UnhashMap`, the one used in `rustc_metadata`, is a decoder's lookup table.
- `decodedFresh`: before it checked the callee's definition it also flagged
  `reserve_and_set_fn_alloc`, `_vtable_alloc`, `_static_alloc` and `_type_id_alloc`, which
  deduplicate inside. It now flags only the bug.
- `untrackedWhileEncoding`: 16 reads of `sess.opts`. Each option that decides what is
  encoded is `[TRACKED]` (`metadata_crate_hash`, `force_unstable_if_unmarked`,
  `embed_metadata`, `optimize`, `output_types`, `target_triple`). The `[UNTRACKED]` ones only
  print statistics (`meta_stats`), keep temporary files (`save_temps`), prefetch (`jobs`), or
  differ between incremental and non-incremental builds and not within either
  (`incremental`). The second `source_map()` read (encoding spans) reads the same untracked
  file data as the bug, which its fix's fingerprint covers.
  `gather_enabled_denied_partial_mitigations()` looked like the same bug: the
  `-Zallow-partial-mitigations` and `-Zdeny-partial-mitigations` options are `[UNTRACKED]`
  and kept outside the dependency-tracking hash. But the list it encodes holds every
  mitigation kind with its level, which come from tracked options (`stack_protector`,
  `control_flow_guard`); the untracked ones decide only whether rustc reports an error,
  which a build with only `-Zdeny-partial-mitigations` changed between sessions confirms.
  `proc_macro_quoted_spans()` is session state that expansion rebuilds in every session.
  `sess.target` is tracked, `dcx()` and `prof` only report, and the hit in
  `rustc_sanitizers` is a type-id encoder, not metadata: the query's notion of an encoder
  is a name.

## What Ur would need to do this better

The queries are syntactic. Ur's [rewriting design](https://github.com/PowderworksCode/codebase/blob/main/projects/ur-docs/ur/rewrite.md)
plans most of what they lacked:

- **Types** (its section 4). "A value of a hash-ordered type reaches an encoder" is the
  real property; without types, the queries approximate it by fields of types that derive
  encoders and by aliases, and the triage above did the rest by hand.
- **Names across files** (section 3). `decodedFresh` decides whether a callee deduplicates
  by looking for its definition in the same file.
- **Properties as queries** (section 9). These are that section's checks, run as
  classifiers. A check that fails when its count changes would let CI run them on every
  nightly.
- **Matching inside names.** A method name cannot be a pattern hole, and `contains` on
  unparsed text matched `LazyValue` for the alias `Value`.

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
| `sourceContentRead` | a read of a source file's contents or their hash (`src`, `src_hash`, `external_src`, `checksum_hash`, `unnormalized_source_len`) | reused codegen units keep an edited file's old checksum and embedded source ([report](hunt/issue-stale-debuginfo-source.md)) | 58 | the bug, the same bug in the Cranelift backend, and the metadata site of the stale-metadata bug; the rest are the source map itself, the lexer, dep-info written fresh each session, debugger visualizers (an edited script reaches the rebuild), and fields named `src` that are not source files |

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

# Closed bugs as queries

The same approach, applied to bugs rustc has already fixed: the bugs behind mirth's
properties ([`motivating.md`](motivating.md)). [`ur/rustc/ClosedBugs.rsc`](../ur/rustc/ClosedBugs.rsc)
has one query per bug's pattern, each generalized as far as it still finds the code its
fix changed. `mirth-lab verify-closed <ur binary>` checks that: for each bug it
fetches the files the fixing PR changed, as they were before the fix, runs the query, and
looks for the site. It needs `ur rewrite --classify --report`, which the current Ur
(`~/.ur/bin/ur`, 2026-10-10) no longer has; the table below is from the last Ur that did.

| bug | query | the pattern | finds the fixed site |
|---|---|---|---|
| #82920 | `sortByDefId` | sorting or deduplicating by `DefId`, which is not stable across sessions | `dedup_by_key` in `conv_object_ty_poly_trait_ref` |
| #89598 | `contextCache` | a cache in interior-mutable state on a context, outside the dependency graph | `GlobalCtxt.vtables_cache` |
| #84252 | `untrackedCrateStore` | reading the crate store directly | the `has_global_allocator` provider |
| #40364 | `envRead` | reading an environment variable | `env::var` in `expand_env` |
| #111227, #111295 | `fileRead` | reading a file | `std::fs::read` in `check_for_debugger_visualizer` |
| #45841 | `writeInPlace` | creating an output file directly instead of renaming a finished one into place | `fs::File::create(out_filename)` in `emit_metadata` |
| #117254 | `encoderNotFinished` | a function that creates a `FileEncoder` and never finishes it | `encode_metadata_impl` |
| #119456 | `writeErrorNotFatal` | a failed write reported with `emit_err`, after which compilation goes on | `encode_metadata` |
| #34902 | `hashIterated` | iterating a hash-ordered collection declared in the same file | `xrefs.into_iter()` in `encode_xrefs` |
| #65036 | `hashOrderAlias` (in `RoundTrip.rsc`) | an alias for a hash-ordered collection | `Resolutions = FxHashMap` |
| #66955 | `optionRead` | reading an option, joined afterwards with the options marked `[UNTRACKED]` | `remap_path_prefix` |

These cover 13 of the 27 bugs, and #159677 is the first section's `hashOrderAlias`. The
others were not written as queries:

- **Writer and reader out of step** (#122859, #130201, #144004): a table one crate reads
  and another never writes. The check compares the tables declared in `rmeta/mod.rs`, the
  `record!` calls in the encoder and the providers in `cstore_impl.rs`, which are all
  inside macro invocations, which Ur does not parse into trees yet.
- **Not a pattern in the source**: the parallel front end's nondeterminism (#129094,
  #140413, #150451), the new trait solver accepting overlapping impls incrementally
  (#135514), inline assembly from a failed session leaking into a later one (#139407),
  diagnostics deduplicated wrongly with incremental compilation (#162901), and temporary
  files left after an error (#107001, #139899).
- **Outside rustc's source**: #138678's randomly seeded map was inside `pulldown-cmark`;
  #68149, a dependent opening a dependency's in-flight `.rlib` while searching for crates,
  depends on timing between processes.
- #114669 is the feature metadata reuse came from, not a bug.
Getting all eleven to find their site took four changes to the queries and one to the
inputs: arguments and match patterns are lists in Ur's trees, so the queries read their
text; a provider is labelled by the `Providers` field it is given as; `hashIterated` also
reads parameters; `optionRead` also reads `sopts` and `self` inside `impl Options`; and
the old unstable `crate` visibility, which Ur's grammars lack, is rewritten to `pub(crate)`.

## What they found in today's compiler

On rustc `ea137335b`'s `compiler/` they run in 2.3 seconds and report 1,400 sites. Not
every site was triaged; what was:

**Three untracked options that change reused output.** `optionRead`, joined with the
options marked `[UNTRACKED]`, gave 74 such options read outside the session and the
driver. Most only affect linking, which runs every session, or debugging output. To test
the rest without judging each by hand, [`mirth-lab audit-options`](../crates/mirth-lab/src/tools/audit_options.rs)
builds a crate incrementally without an option, then with it, and compares with a clean
build that has it, as a comment on rust-lang/rust#84232 ("Audit all UNTRACKED options",
open since 2021) suggests. Of 50 boolean options, three change what an incremental session
reuses, on the official nightly as on the patched compiler:

| option | a clean build | the incremental rebuild |
|---|---|---|
| `-Zemit-stack-sizes` | 42 of 44 objects get a `.stack_sizes` section | the previous objects, without it |
| `-Zcodegen-source-order` | one object's code in source order | the previous object |
| `-Zbuild-sdylib-interface` | an interface without function bodies: different metadata, 44 different objects, 27 warnings | the previous full build, 5 warnings |

`-Csave-temps` also writes no temporary files for reused codegen units, which is expected
for a debugging option. Values of `-Ccodegen-units`, `-Zthreads` and three other
value-taking options made no difference. Report draft:
[`hunt/issue-untracked-options.md`](hunt/issue-untracked-options.md).

**Checked and fine:**

- `untrackedCrateStore`: every query provider that reads the crate store is `eval_always`.
- `encoderNotFinished`: the three functions hand their encoder to code that finishes it.
- `writeErrorNotFatal` in `rustc_metadata/src/fs.rs`: reports a failure to copy metadata
  to standard output, where there is no published file.
- The offload manifest that the mono-item collector reads (`fileRead`) is recorded in
  dep-info, and its query, `collect_and_partition_mono_items`, is `eval_always`.
- `sortByDefId`: four of five sites order diagnostics, debug output or suggestions.

**Leads, not confirmed:**

- `dependency_formats` is not `eval_always` but reads `CStore::injected_panic_runtime()`
  directly, #84252's pattern. Triggering it would need the injected panic runtime to change
  while every tracked input stays the same, which normally changes the crate list too.
- `specialization_graph_provider` sorts trait impls by `CrateNum` and `DefIndex`, #82920's
  pattern; its comment says the order is deliberate.

**Not triaged:** most of `envRead` (118), `fileRead` (49), `writeInPlace` (29),
`contextCache` (26), `hashIterated` (14), and the other `writeErrorNotFatal` sites (23).

# Reporting untracked reads inside rustc

Every incremental bug found here so far comes down to one mistake: while computing
something incremental compilation may reuse, rustc read state that the dependency graph
does not track. When that state changes and nothing tracked does, the old result is reused.

| bug | what was read | while computing |
|---|---|---|
| [stale metadata](hunt/issue-stale-metadata-reuse.md) | source file hashes and lengths | metadata |
| [untracked options](hunt/issue-untracked-options.md) | `-Zno-leak-check`, `-C extra-filename`, `-Zemit-stack-sizes`, ... | type checking, metadata, codegen units |
| [stale debuginfo checksum](hunt/issue-stale-debuginfo-source.md) | `SourceFile::src_hash`, source text | codegen units |

The [reuse check](shadow-mode.md) catches such a bug once an edit has made a reused result
stale. [`hunt/report-untracked.patch`](hunt/report-untracked.patch) catches the read itself, on
any build.

## The patch

Applied after the three fixes and [`verify-reuse.patch`](hunt/verify-reuse.patch); on under
`RUSTC_REPORT_UNTRACKED`.

- **The hook.** `rustc_data_structures::untracked::untracked_read(what)` is called where
  untracked state is read. When reporting is on, it looks at the current task: if the read
  happens inside a task whose result can be reused (an ordinary query, metadata, a codegen
  unit; not `eval_always`, not ignored), it prints

  ```text
  rustc-untracked-read: <what>, read at <file:line:col>, while computing `<task kind>`, whose result incremental compilation may reuse
  ```

  once per distinct line. Each task records its kind for this (`TaskDeps::kind`).
- **Declarations.** A task that tracks the state another way says so with
  `rustc_middle::dep_graph::declare_untracked_input(what)`, and its reads of it are not
  reported. The fix for the stale metadata declares "source file contents" right after
  reading the query that fingerprints the source files; without that fix, the same reads
  would be reported. `RUSTC_REPORT_UNTRACKED=all` reports declared reads too, marked
  `rustc-untracked-read-declared`.
- **Where it is called:**
  - every read of an `[UNTRACKED]` option: `options!` generates a `read_<option>()` accessor
    that calls the hook for untracked options, and the 166 places that read one were
    rewritten mechanically to use it, as `(*sess.opts.unstable_opts.read_dump_mir())`;
  - reads of source file contents: codegen's debuginfo (checksum, embedded source) and the
    metadata's source map;
  - the crate store's crate-wide state (`injected_panic_runtime` and the allocator flags).
  - source text read through the source map (see below).
- **Options that only make debugging output** (MIR dumps, statistics, extra validation) are
  not reported: a result reused without them is still correct. The list, with a reason for
  each, is `DEBUGGING_OPTIONS` in `rustc_middle::dep_graph`.

## What it reports

A build of `fixtures/sink`, with the three fixes:

| read | while computing | verdict |
|---|---|---|
| source file contents (debuginfo checksum) | `CompileCodegenUnit` | finding 6 |
| `-Zno-leak-check` | `typeck_root`, `TraitSelect`, ... | **new**: an incremental rebuild accepts a program a clean build rejects |
| `-C extra-filename` | `Metadata` | **new**: reused metadata names the old value (since 1.90) |
| `-Zemit-stack-sizes`, `-Zcodegen-source-order` | `CompileCodegenUnit` | finding 5 |
| `-Zbuild-sdylib-interface` | `mir_built`, `exportable_items`, ... | finding 5 |
| `-Zfuture-incompat-test` | lint passes | **new**, minor: a testing option that marks every lint future-incompatible; replayed warnings keep the previous session's marking |
| the crate store (`injected_panic_runtime`) | `dependency_formats` | covered: which panic runtime is injected follows from the crate graph (`crates`, `eval_always`), each crate's dependency kind and `-C panic`; declared |

Before the debugging options were left out, the list also had `-Zdump-mir`,
`-Zvalidate-mir` and eight others like them, read in MIR queries.

So on one ordinary build it names every untracked-option bug found by the earlier audit and
query, finds one the audit missed because its test crate never reached the leak check, and
finds finding 6 without an edit or a clean build to compare with. The metadata read that
caused the stale-metadata bug shows up as declared; with the fix reverted it would be
reported.

Building the ten replayed crates at their latest commits (all targets, tests included) adds:

| read | while computing | verdict |
|---|---|---|
| `-Zunstable-options` | lint passes, `check_mod_deathness`, `mir_borrowck` | minor: it decides whether rustc's internal lints run, which matters only to crates that use them |
| `-Ztrim-diagnostic-paths` | lint passes, `typeck_root` | minor: how paths print in a warning; a replayed warning keeps the previous session's form (the same class as `-Zfuture-incompat-test`) |
| `-Zunpretty`, `-Zquery-dep-graph` | `trimmed_def_paths` | benign: they only decide whether to record that trimmed paths were computed |

The fuzzer on `fixtures/sink`, serde and regex adds two more groups, neither a new bug:

- `-Zincremental-verify-ich` and `-Zquery-dep-graph`, read in nearly every query: they are
  incremental compilation's own checking and debugging switches (the fuzzer passes the
  first), deciding whether to verify or record, not what to compute.
- `-Zcheck-cfg-all-expected`, `-Zidentify-regions`, `-Zui-testing` and
  `-Zwrite-long-types-to-disk`, read in lint passes and type checking: like
  `-Ztrim-diagnostic-paths`, they change only how a warning is worded, so a replayed
  warning keeps the previous session's wording. Minor, and all but the first are for
  rustc's own tests.

Environment variables and files are read almost only before the first query (option
parsing, crate loading) or while linking, which happens again on every build; the few
exceptions (a manifest for GPU offload, the Apple deployment target) were not hooked.

## Top-level options and other session state

The `Options` struct's own untracked fields (`--extern`, `-L`, the sysroot, `-Z threads` as
`jobs`, the diagnostic width and others) get the same accessors, and 80 reads were rewritten
to them; reads of a proc macro's quoted spans are reported too. Over `fixtures/sink` and the
ten crates, built normally and with `-Zthreads=4`, inside reusable tasks:

| read | where | verdict |
|---|---|---|
| `jobs` | the query system, in every query | benign: decides whether to check the cache before starting a query on a parallel front end |
| `incremental` | MIR inlining, cross-crate inlinability, partitioning, metadata | benign: only whether incremental compilation is on, which cannot change between two incremental sessions |
| `cli_forced_codegen_units`, `cli_forced_local_thinlto_off` | the number of codegen units | benign: partitioning is redone every session (and changing `-C codegen-units` gave the same output in the option audit) |
| proc macro quoted spans | `Metadata` | covered: positions in the proc-macro crate's own files, which the source-file fingerprint of the stale-metadata fix tracks |

Nothing new. The compiler patches are kept locally (`~/mirth-work/patches`), not in this
repository: they are LLM-generated test instruments, not meant for contribution.

## Source text

Spans are tracked by position, not by the text they cover. `SourceMap::span_to_source`, under
`span_to_snippet` and the other text helpers, reports reads of "source text" (the helpers are
`#[track_caller]`, so the report names their caller). Over `fixtures/sink` and the ten
crates, inside reusable tasks:

| read at | while computing | what it does with the text |
|---|---|---|
| `rustc_hir_typeck/src/loops.rs` | `typeck_root` | finds `break` in a `break` expression to place a label; used only for an error |
| `rustc_hir_typeck/src/pat.rs` | `typeck_root` | trims a pattern's span at `&` or a binding mode, stored in the type-check results for the edition-2024 pattern migration lint |
| `rustc_trait_selection/src/traits/dyn_compatibility.rs` | `dyn_compatibility_violations` | finds `(` in a method signature to place a suggestion |
| `rustc_lint/src/unused/must_use.rs` | `lint_mod` | checks whether the text before an expression ends with `let _ =`, to word the warning |
| `rustc_mir_build/.../check_match.rs` | `check_match` | finds the `else` keyword for a suggestion |
| `rustc_metadata/src/rmeta/encoder.rs` | `Metadata` | renders a constant's literal as written, for documentation; covered by the source-file fingerprint with the stale-metadata fix (declared) |

Each computes a position or a wording from raw text. The text can change without anything
tracked changing only when the change keeps every token and position, such as a comment
rewritten to the same length inside the span, so these can go stale but hardly will. Not
reported upstream.

Over the UI suite (`diag-check --compiler-checks` and `ui-incr`, [`checks.md`](checks.md), fourth
batch), 46 more places read source text, in 15 queries, nearly all to word or place an error or
a lint; none is a new kind of read.

## Printing modes inherited from the caller

Printing has thread-local modes (`with_no_trimmed_paths!`, `with_reduced_queries!` and
seven more), and a query runs on the thread of whatever forced it. A query that starts with
one of these set, and prints, could compute a result that depends on who asked first. Each
task records the modes it started with, and the modes' getters report a read of an inherited
one.

On `fixtures/sink`, 18 kinds of query start with an inherited mode (16 with
`with_reduced_queries`, 2 with `with_no_trimmed_paths`), but none reads it, there or in the
ten crates.

## Limits

- Only reads that call the hook are seen. Options and source files are covered; other
  global state (environment variables, files read directly, thread-locals) is not yet.
- It sees the read, not whether the value reached the result. A read whose value cannot
  change the result needs a declaration or an entry in the allow-list, with the reason.
- Code that runs on another thread outside the task, such as LLVM's own work, is not seen.

## In the runs

`mirth-lab fuzz` and `mirth-lab replay` set `RUSTC_REPORT_UNTRACKED` with the reuse check and
keep every distinct line in `<work>/untracked.txt`, so a code path that only a real crate
reaches adds a line.

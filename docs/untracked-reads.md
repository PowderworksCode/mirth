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
| `-Zfuture-incompat-test` | lint passes | a testing option for the future-incompatibility report; switching it between sessions did not change the output |
| the crate store (`injected_panic_runtime`) | `dependency_formats` | covered: which panic runtime is injected follows from the crate graph (`crates`, `eval_always`), each crate's dependency kind and `-C panic`; declared |

Before the debugging options were left out, the list also had `-Zdump-mir`,
`-Zvalidate-mir` and eight others like them, read in MIR queries.

So on one ordinary build it names every untracked-option bug found by the earlier audit and
query, finds one the audit missed because its test crate never reached the leak check, and
finds finding 6 without an edit or a clean build to compare with. The metadata read that
caused the stale-metadata bug shows up as declared; with the fix reverted it would be
reported.

## Limits

- Only reads that call the hook are seen. Options and source files are covered; other
  global state (environment variables, files read directly, thread-locals) is not yet.
- It sees the read, not whether the value reached the result. A read whose value cannot
  change the result needs a declaration or an entry in the allow-list, with the reason.
- Code that runs on another thread outside the task, such as LLVM's own work, is not seen.

## In the runs

`rustc/fuzz.py` and `rustc/replay.py` set `RUSTC_REPORT_UNTRACKED` with the reuse check and
keep every distinct line in `<work>/untracked.txt`, so a code path that only a real crate
reaches adds a line.

# Plan: testing rustc's crate metadata with mirth

## Goal

Show that properties of how rustc writes and reads crate metadata (`.rmeta`)
can be written down, checked on every build, and kept current by blessing.
Then show a series of small, plausible edits to rustc's metadata code, and
which property each one breaks.

Everything runs the way a contributor would run it: `cargo build` on small
projects, with a compiler built through mirth. A build is many `rustc`
processes, and the properties are checked across all of them.

## How it works

1. **An instrumented compiler.** rustc is built with mirth as the wrapper
   (bootstrap's `RUSTC_WRAPPER_REAL` hook). The plugin rewrites the MIR of
   `rustc_metadata` and of the query entry points to call a small runtime
   crate at the sites the configuration names:
   - entering and leaving a *frame*: a query, or `encode_metadata`;
   - *calls* to functions of interest: `std::fs`, `std::env`, `std::time`,
     the table encoders and decoders;
   - *touches* of mutable or interior-mutable statics and thread-locals;
   - *points* where a test can stop or crash the process.
   Arguments the properties need are captured with the call: integers, and
   `&str` and `&Path`, to name crates and files. Table names come from each
   call site's source span when the report is written, so they cost nothing
   at run time.
2. **Recording.** `mirth record -- cargo build` runs a build with that
   compiler. Each `rustc` process writes its events, with monotonic
   timestamps, to its own log.
3. **Reporting.** `mirth report` merges the logs into one list per process,
   with hashes, paths and process ids normalized. The lists are checked in
   and blessed like `.stderr` files: a change in what rustc does with
   metadata shows up as a diff in review.
4. **Properties.** `mirth check` checks the properties below over the merged
   record.

The fixture projects are compiled normally. Only the compiler is
instrumented.

## A blessed list

```text
== rustc dep (lib, metadata+link) ==
frame encode_metadata
  encode  tables.def_kind              x41
  encode  tables.fn_sig                x6
  encode  tables.lookup_deprecation    x1
  create  deps/rmeta*/lib.rmeta              (temp dir)
  rename  deps/rmeta*/lib.rmeta -> deps/libdep-#.rmeta
  remove  deps/rmeta*/

== rustc app (bin) ==
  open    deps/libdep-#.rmeta               after dep:rename
frame fn_sig(dep)                    decode tables.fn_sig            x3
frame def_kind(dep)                  decode tables.def_kind          x12
frame lookup_deprecation_entry(dep)  decode tables.lookup_deprecation x1
```

## Properties

| | property | guards against |
|---|---|---|
| P1 | an `.rmeta` reaches its final path only by a rename from a temporary file beside it | torn files |
| P2 | every open of an `.rmeta` happens after its writer renamed it into place | races between processes |
| P3 | every table entry a dependent reads was written by the crate's writer | silent defaults |
| P4 | metadata is read only inside a query frame, and encoding reads no environment variable, clock or random state outside a list where each entry has a reason | state incremental compilation cannot see |
| P5 | two clean builds give identical `.rmeta` bytes and identical lists | nondeterminism |
| P6 | an incremental rebuild after an edit to a fixture gives the same `.rmeta` and lists as a clean build | incremental bugs |
| P7 | nothing is left in the target directory beyond the expected outputs | dangling files |

## Edits that break it

Each is a small patch to rustc's metadata code, of the kind a contributor
might write:

| # | edit | caught by |
|---|---|---|
| 1 | write the `.rmeta` straight to its final path, without the temporary file and rename | list diff, P1 |
| 2 | drop one `record!` (deprecation) | P3: the dependent silently gets "not deprecated" |
| 3 | read an environment variable or the clock while encoding | P4, P5 |
| 4 | collect entries through a `std` `HashMap` before encoding them | P5 |
| 5 | decode every item of a dependency when it is loaded | list diff (x12 becomes x400) |
| 6 | read a dependency's metadata straight from the crate store, not through its query | P4, P6 |
| 7 | keep the metadata temporary directory | P7 |

For each edit, the relevant existing rustc suites (`tests/incremental`, the
metadata `run-make` tests, part of `tests/ui`) also run against the same
patched compiler without instrumentation, to show which edits they catch.

## Pull requests

1. Skeleton: the plugin library, an example plugin tested through Cargo, CI
   on Linux, macOS and Windows.
2. The runtime and the instrumenting plugin: frames, calls, touches, points,
   argument capture; tested on a fixture through Cargo.
3. The instrumented compiler: pinned rustc commit, setup and build scripts.
4. `mirth record`, `mirth report`, blessing.
5. The properties, `mirth check`.
6. The edits, each with the failure it produces.
7. The writeup, with the comparison against the existing suites.

## Platforms

mirth and its runtime work on Linux, macOS and Windows. The runtime's one
dependency outside `std` is the C runtime's `atexit`, which all three have. The instrumented compiler is built and the demo run on Linux first;
the other two follow once the demo works.

## Pins

nightly-2026-10-06, and the rustc commit it was built from, `ea137335b`. The
instrumented compiler is built from that commit with that nightly as stage 0,
so the injected runtime and the compiler agree about `std`.

# Incremental rebuilds reuse stale metadata when an edit moves no span

<!-- Draft issue for rust-lang/rust. Regression from #114669 (1.90). -->

When an edit changes a source file without changing any query result (a comment added at
the end of a file, for instance), an incremental rebuild republishes the previous
session's `.rmeta` unchanged. That metadata describes the old file: its source map has the
old content hash and length. A clean build of the same source encodes the new ones.
Dependents then fail to find the source, so diagnostics that point into the dependency lose
their snippet.

### Reproduction

```sh
printf 'pub fn f(a: u32) -> u32 { a }\n' > dep.rs
rustc --edition 2024 --crate-type lib --crate-name dep --emit=metadata,link -C incremental=incr --out-dir rebuilt dep.rs
cp rebuilt/libdep.rmeta before.rmeta
printf '// a comment at the end\n' >> dep.rs
rustc --edition 2024 --crate-type lib --crate-name dep --emit=metadata,link -C incremental=incr  --out-dir rebuilt dep.rs
rustc --edition 2024 --crate-type lib --crate-name dep --emit=metadata,link -C incremental=clean --out-dir clean   dep.rs
cmp before.rmeta rebuilt/libdep.rmeta   # identical: the old metadata was reused
cmp rebuilt/libdep.rmeta clean/libdep.rmeta   # differ
```

I expected the rebuilt metadata to equal the clean build's. Instead it is the previous
session's, byte for byte. The clean build differs in the source map entry for `dep.rs`:
its length, line table and content hash.

What a dependent sees:

```rust
// user.rs
pub fn use_it() -> u32 { dep::f() }
```

```sh
rustc --edition 2024 --crate-type lib --crate-name user --extern dep=rebuilt/libdep.rlib user.rs
```

Built against the incrementally rebuilt `dep`, the note has no snippet and a different
column:

```text
note: function defined here
 --> dep.rs:1:7
```

Built against the clean `dep`:

```text
note: function defined here
 --> dep.rs:1:8
  |
1 | pub fn f(a: u32) -> u32 { a }
  |        ^
```

### Root cause

Since #114669 ("Make metadata a workproduct and reuse it", merged 2025-07-04), metadata is
encoded inside a dep-graph task, and a later session reuses the saved file when that node
can be marked green (`encode_metadata`, `rustc_metadata/src/rmeta/encoder.rs`). The node's
dependencies are the queries read while encoding. But `encode_source_map` reads the
`SourceFile`s straight from `tcx.sess.source_map()`, which is not a query, so the encoded
names, lengths, line tables and content hashes are not dependencies. An edit that changes
no query result, such as a comment after the last item or text inside a comment that keeps
every span where it was, leaves the node green. The old file is reused and describes source
that no longer exists.

Bisection: `nightly-2025-07-03` (`667787527`) rebuilds the metadata;
`nightly-2025-07-06` (`5adb489a8`) reuses it. 1.88.0 and 1.89.0 are not affected;
1.90.0 to the current nightly are.

### It happens in real histories

Replaying git histories with an incremental rebuild per commit, compared with a clean build
each time, hit this on ordinary commits. The rebuilt `.rmeta` differs from the clean one only
in the header hash and the content hash of the edited file:

- `memchr`, 5 consecutive commits in 2018 (e.g. `49ab2ef`, "fallback: fix variable name in
  docstrings": the edited text is in a module compiled out on x86_64);
- `smallvec`, 2 commits (e.g. `01354f1`, three lines changed in `lib.rs`);
- `hashbrown`, 1 commit (`957b590`, one line in `src/raw/mod.rs`).

### Suggested fix

Make the metadata task depend on the source files it encodes. The attached patch adds an
`eval_always` query, `local_source_files_fingerprint`, which hashes every local
`SourceFile`'s name, length and content hash. It also reads that query at the start of the
metadata task. An `eval_always` query runs again in every session, but a node depending on
it stays green when its result is unchanged, so metadata is still reused when the files are
truly unchanged. With all three changes proposed in this series applied (this one and the two in #… and
#…), each attached regression test passes, and each fails when only its own change is
removed. These rustc tests still pass: `tests/incremental` (180), the UI tests in
`tests/ui/{deprecation,crate-loading,rmeta,extern,cross-crate}` (532), 46 metadata-related
`tests/run-make` tests, `tests/ui/{consts,statics,const-generics}` (1844) and
`tests/codegen-llvm` (1122). The full test suite was not run. FUZZ_NUMBERS

A regression test in the style of `tests/run-make` is attached
(`incr-metadata-stale-source/rmake.rs`). It fails on the current nightly and passes with
the change.

### How it was found

[mirth](https://github.com/PowderworksCode/mirth) checks properties of rustc's metadata
handling across Cargo builds. One is that an incremental rebuild after an edit encodes the
same metadata as a clean build of the edited source. A fuzzer making random edits to a test
workspace and a replay of crates' git histories both reported it.

### Meta

`rustc --version --verbose`:
```
rustc 1.101.0-nightly (ea137335b 2026-10-05)
binary: rustc
commit-hash: ea137335b78829b4514bf1b4c16302f74fab8581
host: x86_64-unknown-linux-gnu
```

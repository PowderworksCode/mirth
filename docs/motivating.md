# The bugs behind each property

mirth's properties were designed from first principles: what must hold for rustc's
handling of metadata and incremental compilation to be correct. This document records the
real rust-lang/rust bugs that violated each one, so a maintainer can see why a check exists.
Each bug was **reproduced on a toolchain from before its fix and shown fixed on one after**,
with no mirth involved: the original bug, as users met it.

The bugs were found by searching rust-lang/rust for each property; every issue and PR was
read, and every reproduction run on this machine (Linux x86_64). Reproduce one with
`docs/motivating/run.py <issue>`; the outputs used here are in `docs/motivating/out/`.

Some bugs appear under two properties. Three found by mirth itself
([`hunt/`](hunt)) are listed under P6 at the end.

## P1: An `.rmeta` reaches its final path only by a rename of a fully written file

A reader that opens a half-written or truncated `.rmeta` misreads it, usually as an ICE in the decoder. rustc writes the metadata to a temporary directory and renames it into place; P1 checks that protocol on every process. The first bug below is where the protocol came from; the other two published a truncated file through the rename.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#45841](https://github.com/rust-lang/rust/issues/45841) ICE: index out of bounds in libserialize/leb128.rs | [#45899](https://github.com/rust-lang/rust/pull/45899) | 2017-11-18 | `nightly-2017-11-17` → `nightly-2017-11-20` | reproduced | — |
| [#117254](https://github.com/rust-lang/rust/issues/117254) FileEncoder delayed error reporting is still broken | [#117301](https://github.com/rust-lang/rust/pull/117301) | 2023-11-26 | `nightly-2023-11-26` → `nightly-2023-11-28` | reproduced | — |
| [#119456](https://github.com/rust-lang/rust/issues/119456) ICE 'range start index ... out of range for slice of length 16384': rmeta I/O er | [#119510](https://github.com/rust-lang/rust/pull/119510) | 2024-01-03 | `nightly-2024-01-01` → `nightly-2024-01-05` | reproduced | — |

**#45841.** Before the fix, rustc_trans::back::link::emit_metadata did File::create(out_filename) + write_all straight into the final lib<name>.rmeta path (O_WRONLY|O_CREAT|O_TRUNC). Another rustc that was searching the same -L directory could open the file while it was truncated or only partly written, and then panicked decoding it. The issue thread (arielb1: "the compiler writing metadata in parts, so that another instance of the compiler can read metadata while it is being partially written to. Should be fixable by doing an atomic rename") and PR #45899 ("atomically write .rmeta outputs to avoid races ... write a temporary file and then rename it") record exactly this. The fix added the rmeta* tempdir inside the output directory plus fs::rename, and the comment "To avoid races with another rustc process scanning the output directory..." is still in compiler/rustc_metadata/src/fs.rs today. This is the bug that P1 itself comes from.

*This run:* before: the .rmeta is rewritten in place (same inode), and 2 of 1,704 concurrent readers hit the issue's ICE in leb128.rs. After: replaced by rename, 0 of 1,712. Outputs: [`before`](motivating/out/45841/before.txt), [`after`](motivating/out/45841/after.txt).

*Expected, from the issue and the research:* nightly-2017-11-17 (rustc 1.23.0-nightly d0f8e2913 2017-11-16): run.sh prints the same inode before and after the rebuild, then "BUG: liba.rmeta rewritten in place" and "BUG: old hard link sees new bytes". strace shows openat("st/liba.rmeta", O_WRONLY|O_CREAT|O_TRUNC) with no rename. nightly-2017-11-20 (5041b3bb3 2017-11-19): the inode changes, then "OK: liba.rmeta replaced by rename" and "OK: old hard link still holds previous metadata". strace shows the write going to st/rmeta.XXXX/rust.metadata.bin, followed by rename(... , "st/liba.rmeta"). race.sh on 2017-11-17 gave 1 failure in 1078 reader runs in one 60s attempt and 0 in another 90s attempt. The failure was the exact ICE from the issue: "thread 'rustc' panicked at 'index out of bounds: the len is 2432992 but the index is 6805837', /checkout/src/libserialize/leb128.rs:59:20". On 2017-11-20: 0 failures in 1719 runs.

**#117254.** rmeta encoding never called FileEncoder::finish, so an I/O error while writing the temp file (ENOSPC in crater, EFBIG here) was swallowed. rustc then renamed the short temp file to the final lib<name>.rmeta and exited 0. Downstream crates ICE'd in MemDecoder ("We're going ahead to decode a result which was not completely written out", issue text). The rename itself is atomic, but the file it publishes is not fully written, which breaks the second half of P1. Introduced by the delayed error scheme of #94732 and not fixed by #115542. #117301 added the finish() check, but only with emit_err; see the next entry for the remaining hole.

*This run:* before: a write error is swallowed (exit 0), a 1 MiB truncated .rmeta is published, and a dependent ICEs. After: an error and exit 1, but the truncated file is still published. Outputs: [`before`](motivating/out/117254/before.txt), [`after`](motivating/out/117254/after.txt).

*Expected, from the issue and the research:* nightly-2023-11-26 (1.76.0-nightly f5dc2653f 2023-11-25): no diagnostic, "rustc exit=0", and out/libbig.rmeta is exactly 1048576 bytes, a truncated file at the final path. The downstream rustc then ICEs in rustc_serialize/src/opaque.rs ("range start index ... out of range for slice of length 1048576"). nightly-2023-11-28 (49b3924bd 2023-11-27): rustc now prints "error: failed to write to `.../rmetaXXXX/lib.rmeta`: File too large (os error 27)" and exits 1. However, the 1048576-byte out/libbig.rmeta is still published, and the downstream rustc still panics with "range start index 14979595 out of range for slice of length 1048576" (fixed fully by #119510 below).

**#119456.** After #117301, rmeta write errors were reported with emit_err, which does not stop compilation. rustc kept going and renamed the incomplete temp file over lib<name>.rmeta, and cargo could start a dependent build that read it (PR #119510: "there is a window of time between the call to emit_err and the full error reporting where rustc believes it has emitted a valid rmeta file and will permit Cargo to launch a build for a dependent crate"). Switching to emit_fatal aborts before the rename, so nothing partial reaches the final path. The reporter hit this on 1.75.0 with typst as a dependency (disk-full); the PR author reproduced it with an LD_PRELOAD write() that randomly returns ENOSPC.

*This run:* before: an error and exit 1, yet the truncated .rmeta is published and a dependent ICEs. After: nothing is published, and the dependent gets `can't find crate`. Outputs: [`before`](motivating/out/119456/before.txt), [`after`](motivating/out/119456/after.txt).

*Expected, from the issue and the research:* nightly-2024-01-01 (1.77.0-nightly e51e98dde 2023-12-31): "error: failed to write to `.../rmetaXXXX/lib.rmeta`: File too large (os error 27)" and exit 1, yet `ls -l out` shows libbig.rmeta at 1048576 bytes. The reader then panics: "thread 'rustc' panicked at .../compiler/rustc_serialize/src/opaque.rs:262:42: range start index 12750977 out of range for slice of length 1048576". nightly-2024-01-05 (f688dd684 2024-01-04): same error and exit 1, but out/ is empty (the truncated file is never renamed into place), and the reader gets a clean "error[E0463]: can't find crate for `big`". Current stable 1.97.1 behaves the same as 2024-01-05 (the temp file is now named full.rmeta).

## P2: No process opens a dependency's `.rmeta` before it has been renamed into place

With pipelining, a dependent starts as soon as its dependency's metadata exists, so the order of writes and opens across processes matters. P2 compares the timestamps of opens in readers with renames in writers.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#45841](https://github.com/rust-lang/rust/issues/45841) ICE: index out of bounds in libserialize/leb128.rs | [#45899](https://github.com/rust-lang/rust/pull/45899) | 2017-11-18 | `nightly-2017-11-17` → `nightly-2017-11-20` | reproduced | — |
| [#68149](https://github.com/rust-lang/rust/issues/68149) Spurious rebuilds under pipelining: a consumer opened | [#68298](https://github.com/rust-lang/rust/pull/68298) | 2020-01-23 | `nightly-2020-01-22` → `nightly-2020-01-24` | reproduced | — |

**#45841.** Before the fix, rustc_trans::back::link::emit_metadata did File::create(out_filename) + write_all straight into the final lib<name>.rmeta path (O_WRONLY|O_CREAT|O_TRUNC). Another rustc that was searching the same -L directory could open the file while it was truncated or only partly written, and then panicked decoding it. The issue thread (arielb1: "the compiler writing metadata in parts, so that another instance of the compiler can read metadata while it is being partially written to. Should be fixable by doing an atomic rename") and PR #45899 ("atomically write .rmeta outputs to avoid races ... write a temporary file and then rename it") record exactly this. The fix added the rmeta* tempdir inside the output directory plus fs::rename, and the comment "To avoid races with another rustc process scanning the output directory..." is still in compiler/rustc_metadata/src/fs.rs today. This is the bug that P1 itself comes from.

*This run:* before: the .rmeta is rewritten in place (same inode), and 2 of 1,704 concurrent readers hit the issue's ICE in leb128.rs. After: replaced by rename, 0 of 1,712. Outputs: [`before`](motivating/out/45841/before.txt), [`after`](motivating/out/45841/after.txt).

*Expected, from the issue and the research:* nightly-2017-11-17 (rustc 1.23.0-nightly d0f8e2913 2017-11-16): run.sh prints the same inode before and after the rebuild, then "BUG: liba.rmeta rewritten in place" and "BUG: old hard link sees new bytes". strace shows openat("st/liba.rmeta", O_WRONLY|O_CREAT|O_TRUNC) with no rename. nightly-2017-11-20 (5041b3bb3 2017-11-19): the inode changes, then "OK: liba.rmeta replaced by rename" and "OK: old hard link still holds previous metadata". strace shows the write going to st/rmeta.XXXX/rust.metadata.bin, followed by rename(... , "st/liba.rmeta"). race.sh on 2017-11-17 gave 1 failure in 1078 reader runs in one 60s attempt and 0 in another 90s attempt. The failure was the exact ICE from the issue: "thread 'rustc' panicked at 'index out of bounds: the len is 2432992 but the index is 6805837', /checkout/src/libserialize/leb128.rs:59:20". On 2017-11-20: 0 failures in 1719 runs.

**#68149.** Under cargo pipelining, a consumer starts as soon as the dependency's .rmeta is published, while the dependency's rustc is still producing the .rlib. When the locator resolved a transitive dependency by searching -L dependency=, it opened and remembered the .rlib if it was present, even though an rlib-only build needs just the .rmeta. ehuss traced the timeline: T1 libcore.rmeta emitted; T2 the backtrace build starts; T3 libcore.rlib emitted; T4 backtrace loads libcore.rlib; T5 its dep-info lists both. Cargo backdates the dep-info to T2, so the next build sees rlib mtime T3 > T2 and spuriously rebuilds std crates (reported by petrochenkov with -Zbinary-dep-depinfo in x.py). This is a P2-adjacent violation: a pipelined consumer reaches into a dependency artifact (.rlib) that is not yet finished or published from its point of view. PR #68298 ('Avoid declaring a fake dependency edge') stopped storing rlib/dylib paths when only producing an rlib. Caveat: the file opened is the .rlib, not the .rmeta.

*This run:* before: the consumer opens and records the dependency's in-flight .rlib. After: only the .rmeta files. Outputs: [`before`](motivating/out/68149/before.txt), [`after`](motivating/out/68149/after.txt).

*Expected, from the issue and the research:* Run on this VM. nightly-2020-01-22 (rustc 5e8897b7b 2020-01-21, which does not contain merge be663bf85): c's dep-info lists D/liba-x.rlib as well as liba-x.rmeta and libb-x.rmeta, and strace shows c opening D/liba-x.rlib. nightly-2020-01-24 (rustc 41f41b235 2020-01-23, which contains it): only liba-x.rmeta and libb-x.rmeta are listed and opened. This deterministic check shows the cause: the consumer opening the in-flight rlib. The user-visible symptom, a spurious cargo rebuild, needs the rlib to appear in the small window between the consumer starting and loading crates (ehuss: 'the time between T2 and T4 is extremely small'), plus -Zbinary-dep-depinfo, so the cargo-level symptom is not cheap to reproduce.

## P3: A dependent reads only table entries the dependency wrote

An entry the writer never wrote reads as a default, without complaint, so a table that stops being written shows up as wrong behaviour far away: a missing bound, a missing attribute, or an ICE in the reader. The `written` column of the blessed list records it.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#122859](https://github.com/rust-lang/rust/issues/122859) Implied bound not implied across crates: associated-type bounds in supertrait po | [#122891](https://github.com/rust-lang/rust/pull/122891) | 2024-03-24 | `nightly-2024-03-24` → `nightly-2024-03-26` | reproduced | caught when the fix (#122891) is reverted: the list shows the table no longer written |
| [#130201](https://github.com/rust-lang/rust/issues/130201) ICE: `coroutine_by_move_body_def_id` unsupported by its crate when calling a for | [#130201](https://github.com/rust-lang/rust/pull/130201) | 2024-09-17 | `nightly-2024-09-17` → `nightly-2024-09-19` | reproduced | caught when the fix is reverted: the build ICEs |
| [#144004](https://github.com/rust-lang/rust/issues/144004) rustdoc drops #[no_mangle] / #[link_section] from inlined cross-crate re-exports | [#144050](https://github.com/rust-lang/rust/pull/144050) | 2025-07-19 | `nightly-2025-07-06` → `nightly-2025-07-24` | reproduced | missed when the fix (#144050) is reverted: only rustdoc reads these attributes |

**#122859.** For traits, the metadata encoder assumed `implied_predicates_of` was equal to `super_predicates_of` and only wrote the latter. So the implied predicates that come from associated type bounds (`trait Bar: Super<SuperAssoc: Bound>`) were never written. The dependent crate read a smaller predicate list without noticing, and lost the bound. That shows up as a wrong E0277 that only happens across crates. The PR says: 'The assumption that they didn't differ was hard-coded in #107614, so in cross-crate positions this means that we forget the implied predicates from associated type bounds.' The same code compiles when crate_b is a local module.

*This run:* before: E0277, the implied bound lost across crates. After: compiles. Outputs: [`before`](motivating/out/122859/before.txt), [`after`](motivating/out/122859/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2024-03-24, main.rs fails with `error[E0277]: the trait bound `<<T as Foo>::FooAssoc as Super>::SuperAssoc: Unsatisfied` is not satisfied`. On nightly-2024-03-26 it compiles (there is only a dead-code warning). Pasting crate_b's contents into main.rs as `mod crate_b` compiles on both, which shows that only the cross-crate (metadata) path is affected.

**#130201.** The dependency crate never wrote the `coroutine_by_move_body_def_id` table entry, and never wrote optimized_mir for the synthetic by-move body. When the dependent crate asked for that entry, the lookup found nothing and fell through to a missing provider, which ICEs. The PR text says: 'We weren't encoding this query in the metadata though, nor were we properly recording that synthetic MIR in `mir_keys`, so the `optimized_mir` wasn't getting encoded either!' There is no separate issue; PR #130201 itself is the reference, and its regression test is tests/ui/async-await/async-closures/foreign.rs.

*This run:* before: the dependent ICEs (`coroutine_by_move_body_def_id` unsupported by its crate). After: compiles. Outputs: [`before`](motivating/out/130201/before.txt), [`after`](motivating/out/130201/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2024-09-17, compiling main.rs ICEs with: `error: internal compiler error: compiler/rustc_middle/src/query/plumbing.rs:664:5: `tcx.coroutine_by_move_body_def_id(DefId(20:6 ~ foreign[87b3]::closure::{closure#0}::{closure#0}))` unsupported by its crate; perhaps the `coroutine_by_move_body_def_id` query was never assigned a provider function`. On nightly-2024-09-19 it compiles cleanly and produces the `main` binary.

**#144004.** `no_mangle` and `link_section` were moved out of the generic encoded attribute list, and nothing else wrote them to the attribute table. A dependent crate that reads the dependency's attributes from metadata (here rustdoc inlining a `pub use a::*` re-export) silently gets no such attribute, with no error. The PR title is 'Fix encoding of link_section and no_mangle cross crate', and it fixes it by always encoding them. This is the 'missing attributes cross-crate' kind of P3 violation. The result is silent information loss, not an ICE.

*This run:* before: rustdoc shows neither attribute on the re-exports. After: `no_mangle` and `link_section` shown. Outputs: [`before`](motivating/out/144004/before.txt), [`after`](motivating/out/144004/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2025-07-06 all four pages (doc/b/fn.f0.html, fn.f1.html, static.S0.html, static.S1.html) show no attribute, so every grep prints nothing. On nightly-2025-07-24 they show `no_mangle`, `link_section = ".here"`, `no_mangle` and `link_section = ".there"`. The issue adds that 1.88.0 stable also lacked both attributes, and 1.89 beta showed no_mangle but not link_section. The regression came and went with attribute-parsing refactors.

## P4: Encoding reads no untracked state

Environment variables, the clock, randomly seeded maps and files that are not declared inputs all reach outputs without incremental compilation or Cargo knowing. P4 records such reads while encoding.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#40364](https://github.com/rust-lang/rust/issues/40364) env!/option_env! values were baked into the output, but the env vars were not re | [#71858](https://github.com/rust-lang/rust/pull/71858) | 2020-06-26 | `1.45.0` → `1.46.0` | reproduced | — |
| [#66955](https://github.com/rust-lang/rust/issues/66955) --remap-path-prefix was UNTRACKED: changing it under incremental reused stale co | [#84233](https://github.com/rust-lang/rust/pull/84233) | 2021-04-29 | `nightly-2021-04-29` → `nightly-2021-05-01` | reproduced | — |
| [#111227](https://github.com/rust-lang/rust/issues/111227) debugger_visualizer files | [#111641](https://github.com/rust-lang/rust/pull/111641) | 2023-05-19 | `nightly-2023-05-18` → `nightly-2023-05-21` | partly reproduced | — |
| [#138678](https://github.com/rust-lang/rust/issues/138678) Randomly seeded HashMap | [#138678](https://github.com/rust-lang/rust/pull/138678) | 2025-03-28 | `nightly-2025-03-28` → `nightly-2025-03-30` | reproduced | caught when the fix is reverted: P5, P5 with threads, the touch rebuild and P6 |

**#40364.** P4: env!() reads the process environment while compiling, and the value ends up in the output. rustc did not record that the variable was read, so build tools treated the crate as fresh after the variable changed. PR #71858 added '# env-dep:KEY=VALUE' lines to dep-info, closing #40364, #44074 and #70517. Cargo then read those lines and rebuilt on a change (rust-lang/cargo PR #8421, merged 2020-06-30). Both parts are needed for the repro, so it uses stable releases with the matching cargo: 1.45.0 has neither part, 1.46.0 has both. rustc nightlies after 2020-06-26 have the dep-info lines; cargo's rebuild arrived when the cargo submodule was next updated in early July 2020.

*This run:* before: changing the variable leaves the binary printing the old value, and dep-info has no env-dep. After: the new value, and `# env-dep:MIRTH_DEMO=two`. Outputs: [`before`](motivating/out/40364/before.txt), [`after`](motivating/out/40364/after.txt).

*Expected, from the issue and the research:* Verified locally. With 1.45.0 the two runs print 'one' then 'one': the stale binary is reused after MIRTH_DEMO changed. With 1.46.0 they print 'one' then 'two'. On 1.46.0 the dep-info file also has a '# env-dep:MIRTH_DEMO=two' line; 1.45.0 has none.

**#66955.** P4, read broadly as untracked state that reaches the output: #48162 made --remap-path-prefix UNTRACKED so that the crate hash stayed the same. Remapped paths still went into the outputs, so after the flag changed, an incremental rebuild reused cached codegen units holding the old prefix. The result was a mix of old and new remapped paths. The fix (#84233) added TRACKED_NO_CRATE_HASH: the option invalidates the incremental cache but stays out of the crate hash. Note: in this era metadata was always re-encoded, so the rmeta itself picked up the new path. The stale data is in the object code and debuginfo inside the rlib.

*This run:* before: after changing the remap, the rlib still holds 2 copies of the old path. After: only the new path. Outputs: [`before`](motivating/out/66955/before.txt), [`after`](motivating/out/66955/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2021-04-29, after the second build (remap to /BBBB_second) the rlib still contains '2 /AAAA_first' and only '1 /BBBB_second'; the stale object code and debuginfo were reused. On nightly-2021-05-01 it contains '3 /BBBB_second' and no /AAAA_first.

**#111227.** P4: the contents of files named by #![debugger_visualizer(natvis_file / gdb_script_file)] were read and encoded into crate metadata (the debugger_visualizers query), but those files were not tracked inputs. They were missing from dep-info (#111226), so cargo never rebuilt after a change. The incremental system also did not see them: changing the natvis file gave 'internal compiler error: encountered incremental compilation error with debugger_visualizers' (#111227), and changed GDB scripts were not picked up (#111295). The fix (#111641) made debugger_visualizers an eval_always query computed from the AST and added the files to dep-info. Its tests include run-make/incremental-debugger-visualizer, which greps the rmeta for the file contents.

*Status:* partly reproduced: `foo.py` is missing from the dep-info before the fix and present after it, but the stale metadata and ICE the issue describes did not appear here.

*This run:* before: the visualizer file is missing from dep-info. After: listed. The stale metadata did not appear here. Outputs: [`before`](motivating/out/111227/before.txt), [`after`](motivating/out/111227/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2023-05-18 the script prints 'foo.py NOT in dep-info'. The second, incremental compile prints 'error: internal compiler error: encountered incremental compilation error with debugger_visualizers(foo[47df])', and libfoo.rmeta still contains 'Natvis v1', so the metadata is stale. On nightly-2023-05-21 foo.py is listed in foo.d, the rebuild succeeds, and the rmeta contains 'Natvis v2'.

**#138678.** P4: metadata encoding read a randomly seeded std HashMap. rustc_resolve::rustdoc::parse_links walks pulldown-cmark's reference_definitions(), a HashMap with RandomState, and pushes the links in that iteration order. The resulting list of doc-link candidates is encoded into crate metadata, so two runs of rustc on the same input wrote different .rmeta bytes. The bug came in with #136363 (merged 2025-02-16). The fix (#138678) sorts the links by label. #138678 is the PR; it has no separate issue, and its description says the nondeterminism was found in Bazel lib.rmeta outputs.

*This run:* before: 8 builds, 8 different .rmeta files. After: 8 identical. Outputs: [`before`](motivating/out/138678/before.txt), [`after`](motivating/out/138678/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2025-03-28 (rustc 1.87.0-nightly 3f5502370 2025-03-27), the 8 identical compilations gave 8 different liblib.rmeta hashes, each counted once. On nightly-2025-03-30 (1.88.0-nightly 1799887bb 2025-03-29), all 8 gave the same hash (count 8).

## P5: Two clean builds give the same bytes

Nondeterminism in metadata breaks reproducible builds and makes crate hashes, and so everything downstream, depend on chance.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#34902](https://github.com/rust-lang/rust/issues/34902) Metadata xrefs encoded in pointer-address | [#35984](https://github.com/rust-lang/rust/pull/35984) | 2016-08-28 | `nightly-2016-08-27` → `nightly-2016-08-30` | reproduced | — |
| [#65036](https://github.com/rust-lang/rust/issues/65036) Module re-exports serialized into metadata in FxHashMap order keyed by | [#65043](https://github.com/rust-lang/rust/pull/65043) | 2019-10-06 | `nightly-2019-10-05` → `nightly-2019-10-08` | reproduced | — |
| [#159677](https://github.com/rust-lang/rust/issues/159677) .rmeta contents depend on unrelated files in the library search path | [#159718](https://github.com/rust-lang/rust/pull/159718) | 2026-07-24 | `nightly-2026-07-20` → `nightly-2026-09-25` | reproduced | not replayed (needs a decoy crate in the search path) |
| [#129094](https://github.com/rust-lang/rust/issues/129094) Parallel frontend: derives make metadata irreproducible | [#161450](https://github.com/rust-lang/rust/pull/161450) | 2026-09-16 | `nightly-2026-07-20` → `nightly-2026-09-25` | reproduced | not replayed (the revert does not apply cleanly) |

**#34902.** encode_xrefs iterated an FnvHashMap<XRef<'tcx>, u32>. Its keys are interned ty::Predicate pointers, so the hash and the iteration order depend on heap addresses, which ASLR changes on every run. The commit 'Make metadata encoding deterministic' in PR #35984 ('Steps towards reproducible builds', cc tracking issue #34902) sorts the xrefs by their ID before encoding. This is the classic single-threaded pointer-address nondeterminism: the same command run twice in the same directory gives different rust.metadata.bin bytes, while the object file stays identical.

*This run:* before: 5 builds, 5 different rlibs (only the metadata member differs); identical with ASLR off. After: 5 identical. Outputs: [`before`](motivating/out/34902/before.txt), [`after`](motivating/out/34902/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2016-08-27 (rustc 1.13.0-nightly 198713106) 10 runs gave 10 different liblib.rlib hashes. Only the rust.metadata.bin member differs; lib.0.o is identical. Under 'setarch -R' (ASLR off) the runs are identical, which confirms that pointer addresses are the cause. On nightly-2016-08-30 (77d2cd28f) all 10 runs give the same hash. Very old toolchain: install it with 'rustup toolchain install nightly-2016-08-27 --profile minimal'.

**#65036.** The PR text says: 're-exports end up getting serialized into crate metadata, which means that metadata generation was non-deterministic'. A module's resolutions were an FxHashMap<(Ident, Namespace), ...>, and Ident hashes by Symbol interner index. Anything that changes interning order changes the order of re-exports in .rmeta. The fix changes Resolutions to an FxIndexMap. The reported symptom (#65036) was flaky diagnostics, std::mem::transmute vs std::intrinsics::transmute. The reproduction below uses the same unrelated-file-in-search-path trigger as #159677. A proc macro interns the item names after the extern crate lookup has interned 'foo_bar', so the indices shift.

*This run:* before: two builds differ at byte 5163, re-exports in a different order. After: identical. Outputs: [`before`](motivating/out/65036/before.txt), [`after`](motivating/out/65036/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2019-10-05 (rustc 1.40.0-nightly 2e7244807 2019-10-04) cmp reports 'differ: byte 5157', and the strings diff shows the item_N re-export names in a different order (item_21, item_1, item_24, item_11, ...). On nightly-2019-10-08 (f3c9cece7 2019-10-07) the script prints IDENTICAL.

**#159677.** Building the same client.rs with the same flags twice gives different .rmeta bytes if an unrelated rlib whose name starts with the dependency's name (libfoo_bar.rlib next to libfoo.rlib) is in the -L directory. The crate locator opens libfoo_bar.rlib to read its crate name, which interns the extra Symbol `foo_bar`. That shifts the interner indices of symbols interned later, such as the doc-link strings. DocLinkResMap was an UnordMap (FxHashMap) keyed by (Symbol, Namespace) and hashed by interner index, and it was encoded in hash-iteration order. The fix makes it an FxIndexMap, so entries are encoded in insertion order. The PR adds the regression test tests/run-make/rmeta-unrelated-search-path-files.

*This run:* before: adding an unrelated library to the search path changes the .rmeta (differs at byte 1256). After: identical. Outputs: [`before`](motivating/out/159677/before.txt), [`after`](motivating/out/159677/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2026-07-20 (and on stable 1.97.1) cmp prints 'client1.rmeta client2.rmeta differ: byte 1256' (1263 on 1.97.1); the differing bytes are the reordered doc-link entry 'crate::Client'. On nightly-2026-09-25 the files are identical and the script prints IDENTICAL. The fix merged 2026-07-24T09:12Z, so nightly-2026-07-26 and later should be fixed.

**#129094.** With -Zthreads=N, the order in which SyntaxContexts and expansions are reached during metadata encoding depends on thread scheduling, so a tiny derive-heavy crate compiled repeatedly with the same inputs gives different rlibs. The fix ('Fix non-deterministic encoding of syntax contexts', which reiterates #157409) adds deterministic encoding indices in rustc_metadata/rmeta/encoder.rs and rustc_span/hygiene.rs. It also adds derives-issue-129094.rs to tests/run-make/parallel-reproducible-build. Unlike the other entries this is not single-threaded: it needs the parallel frontend.

*This run:* before: 20 threaded builds give 4 different rlibs. After: 20 identical. Outputs: [`before`](motivating/out/129094/before.txt), [`after`](motivating/out/129094/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2026-07-20, 20 runs gave 3 distinct rlib hashes (15/4/1). On nightly-2026-09-25 all 20 runs give one hash. Race-dependent: it needs -Zthreads>1 and several runs, and how often it shows up depends on core count and scheduling. It reproduced readily on this VM.

## P5t: Two clean builds with `-Zthreads` give the same bytes

The parallel front end adds a new source of nondeterminism: the order in which threads create and intern things.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#140413](https://github.com/rust-lang/rust/issues/140413) parallel rustc: static mut refs not reproducible | [#144722](https://github.com/rust-lang/rust/pull/144722) | 2025-08-13 | `nightly-2025-07-26` → `nightly-2025-12-10` | reproduced | — |
| [#150451](https://github.com/rust-lang/rust/issues/150451) parallel compiler: thread::spawn-ing loop not reproducible | [#160197](https://github.com/rust-lang/rust/pull/160197) | 2026-09-07 | `nightly-2026-07-20` → `nightly-2026-09-25` | reproduced | — |
| [#129094](https://github.com/rust-lang/rust/issues/129094) Parallel frontend: derives make metadata irreproducible | [#161450](https://github.com/rust-lang/rust/pull/161450) | 2026-09-16 | `nightly-2026-07-20` → `nightly-2026-09-25` | reproduced | not replayed (the revert does not apply cleanly) |

**#140413.** With -Zthreads=50, building the same binary repeatedly gives different bytes. Mono items were sorted by (DefId, SymbolName) to pick their order in the output. DefId indices are allocated in access order, which is nondeterministic under the parallel front end. PR #144722 ('Fix parallel rustc not being reproducible due to unstable sorts of items') stops sorting by DefId. Two later facts confirm the fix: PR #161353 (merged 2026-09-01; it closed the issue and added tests/run-make/parallel-reproducible-build) found by bisection that the regression flips at nightly-2025-08-14, at commit #144722. The same PR also fixes the async-closure case #140425 (closed 2025-08-13).

*This run:* before: 15 threaded builds give 2 different binaries. After: 15 identical. Outputs: [`before`](motivating/out/140413/before.txt), [`after`](motivating/out/140413/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2025-07-26 there were 2 distinct md5s of the binary over 15 runs. On nightly-2025-12-10 there was 1. The bisected boundary is nightly-2025-08-13 (bad) to nightly-2025-08-14 (good). Caveat: building this file as --crate-type=lib --emit=metadata is still nondeterministic, even on nightly-2026-10-06 (7-8 distinct rmeta md5s out of 10). That case is the still-open follow-up #162203, so use this repro for the binary only.

**#150451.** With -Zthreads=3, compiling a tiny lib that uses thread::spawn gives a different .rlib each time. The output differs when bitcode is embedded or LTO is used. The cause is that the srcloc 'cookies' attached to LLVM inline asm were assigned nondeterministically by the parallel front end. PR #160197 ('Restrict LLVM inline asm location cookie usage. Fixes #150451') landed in rollup #162434 on 2026-09-07 and added tests/run-make/parallel-reproducible-inline-asm-cookie. The bug is in object code and bitcode, not in rmeta, but it breaks reproducibility of the published artifact.

*This run:* before: 15 threaded builds give 10 different rlibs. After: 15 identical. Outputs: [`before`](motivating/out/150451/before.txt), [`after`](motivating/out/150451/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2026-07-20 there were 7 distinct rlib md5s over 15 runs. On nightly-2026-09-25 there was 1.

**#129094.** With -Zthreads=N, the order in which SyntaxContexts and expansions are reached during metadata encoding depends on thread scheduling, so a tiny derive-heavy crate compiled repeatedly with the same inputs gives different rlibs. The fix ('Fix non-deterministic encoding of syntax contexts', which reiterates #157409) adds deterministic encoding indices in rustc_metadata/rmeta/encoder.rs and rustc_span/hygiene.rs. It also adds derives-issue-129094.rs to tests/run-make/parallel-reproducible-build. Unlike the other entries this is not single-threaded: it needs the parallel frontend.

*This run:* before: 20 threaded builds give 4 different rlibs. After: 20 identical. Outputs: [`before`](motivating/out/129094/before.txt), [`after`](motivating/out/129094/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2026-07-20, 20 runs gave 3 distinct rlib hashes (15/4/1). On nightly-2026-09-25 all 20 runs give one hash. Race-dependent: it needs -Zthreads>1 and several runs, and how often it shows up depends on core count and scheduling. It reproduced readily on this VM.

## P6: An incremental rebuild gives the same results as a clean build

Incremental compilation is only correct if what it reuses is what it would have computed. When it is not, the result is a stale output, a miscompilation or a wrong diagnostic, which `cargo clean` fixes. These bugs are why P6 compares an incremental rebuild with a clean build of the same source.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#82920](https://github.com/rust-lang/rust/issues/82920) Miscompilation with incr. comp. | [#83074](https://github.com/rust-lang/rust/pull/83074) | 2021-03-15 | `nightly-2021-03-13` → `nightly-2021-03-17` | reproduced | — |
| [#89598](https://github.com/rust-lang/rust/issues/89598) VTable-related miscompilation with incremental compilation | [#89619](https://github.com/rust-lang/rust/pull/89619) | 2021-10-08 | `nightly-2021-10-07` → `nightly-2021-10-10` | reproduced | — |
| [#135514](https://github.com/rust-lang/rust/issues/135514) Rust 1.84 sometimes allows overlapping impls in incremental re-builds | [#133828](https://github.com/rust-lang/rust/pull/133828) | 2024-12-05 | `1.84.0` → `1.85.0` | reproduced | — |
| [#139407](https://github.com/rust-lang/rust/issues/139407) Instructions missing from | [#139453](https://github.com/rust-lang/rust/pull/139453) | 2025-04-11 | `nightly-2025-04-10` → `nightly-2025-04-13` | reproduced | — |
| [#162901](https://github.com/rust-lang/rust/issues/162901) Diagnostic deduplication breaks with incr comp: incremental builds print 4 copie | [#163461](https://github.com/rust-lang/rust/pull/163461) | 2026-10-02 | `nightly-2026-10-01` → `nightly-2026-10-03` | reproduced | — |

**#82920.** Bounds and predicates were sorted by DefId, which is not stable across sessions. When two trait declarations swap places, the query result changes but is still treated as green and reused, so the vtable layout and the method call sites disagree. The incremental binary calls the wrong trait method, while a clean build is correct. The original report was a rust-analyzer test suite failing with out-of-bounds panics and segfaults until `cargo clean`. Labelled I-unsound.

*This run:* before: the incremental pass-2 binary panics (`left: 2, right: 1`) while a clean build of the same source passes. After: both pass. Outputs: [`before`](motivating/out/82920/before.txt), [`after`](motivating/out/82920/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2021-03-13, the incremental pass 2 binary panics with "assertion failed: `(left == right)` left: `2`, right: `1`" (method_two was called for method_one), while a clean rpass2 build prints 'clean ok'. On nightly-2021-03-17 both passes print ok. Regression test: tests/incremental/issue-82920-predicate-order-miscompile.rs.

**#89598.** #86475 added an untracked global vtable cache to tcx. After trait methods are reordered, the object file for the main CGU is reused from the incremental cache with the old vtable layout, while mod1 is recompiled for the new layout. The incremental binary then calls method2 where a clean build calls method1, so the binaries differ (a miscompile). P-critical, I-unsound, regression-from-stable-to-beta.

*This run:* before: the incremental pass-2 binary calls the wrong method (`left: 42, right: 17`); the clean build passes. After: both pass. Outputs: [`before`](motivating/out/89598/before.txt), [`after`](motivating/out/89598/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2021-10-07, pass 1 prints 'pass1 ok'; the incremental pass 2 binary panics with "assertion failed: `(left == right)` left: `42`, right: `17`" because method2 was called through the stale vtable. The clean build of rpass2 runs fine. On nightly-2021-10-10 both passes print ok. Regression test: tests/incremental/reorder_vtable.rs.

**#135514.** A clean build of the edited source fails with E0119 (conflicting implementations). The incremental rebuild after the same edit accepts it, because coherence results computed through the new solver's cache had no dependency edges. The accepted program is a safe transmute: Vec<u8> becomes String and prints ABC. The diagnostics and the success or failure differ from a clean build. Labelled I-unsound, regression-from-stable-to-stable. #135522 added tests/incremental/overlapping-impls-in-new-solver-issue-135514.rs. The fix was already on master before the issue was filed, so 1.85 is fixed and 1.84.x is affected.

*This run:* before: the incremental rebuild accepts overlapping impls and runs, while a clean build gives E0119. After: both give E0119. Outputs: [`before`](motivating/out/135514/before.txt), [`after`](motivating/out/135514/after.txt).

*Expected, from the issue and the research:* Verified locally. On 1.84.0, pass 1 prints 'pass1'; the incremental pass 2 compiles without error and prints 'ABC' then 'incremental pass2 built and ran'; the clean build of the same source fails with error[E0119]: conflicting implementations of trait `Other` for type `S<W>`. On 1.85.0 the incremental pass 2 also fails with E0119. On 1.83.0 the old solver rejects pass 1 itself, so use 1.84.0. For nightlies, try nightly-2024-12-04 (bug) and nightly-2024-12-07 (fixed); these were not run.

**#139407.** Object files were hard-linked from fixed temp paths into the incremental session directory. A session that fails in the assembler (left unfinalized) overwrites a temp file that a previous, finalized session still hard-links. On the next successful build the reused object holds code from the failed session's source, so the binary differs from a clean build. The fix gives temp files a per-invocation random prefix. Labelled I-unsound. The run-make regression test is tests/run-make/dirty-incr-due-to-hard-link. The reporter hit it with plain `cargo run` (no -Csave-temps) on aarch64-apple-darwin. The test, used here, needs -Csave-temps to keep the temp files on x86_64 Linux. It needs a failed build in between (an asm error), but it is deterministic.

*This run:* before: after a failed build is fixed back, the rebuilt binary still contains the failed session's code and panics. After: it passes. Outputs: [`before`](motivating/out/139407/before.txt), [`after`](motivating/out/139407/after.txt).

*Expected, from the issue and the research:* Verified locally on x86_64 Linux. On nightly-2025-04-10 (and on stable 1.70.0, 1.86.0 and 1.87.0), pass 1 prints 'pass1 ok' and pass 2 fails with "error: invalid instruction mnemonic 'missing'". Pass 3 has the same source as pass 1, yet its binary panics with "assertion `left == right` failed left: 1 right: 0": a() from the failed cfail2 session leaked into the reused object. On nightly-2025-04-13 and on 1.88.0, pass 3 prints 'pass3 ok'.

**#162901.** In incremental mode, spans carry a parent (incremental-relative spans). The diagnostic dedup hash included Span::parent, so identical diagnostics hashed differently and were all emitted. The diagnostics from an incremental compile differ from a non-incremental compile of the same source. The fix PR also fixes #106571 (regression from #84762, Jan 2023), where `cargo check --message-format=json` with incremental on prints identical compiler-message lines twice for a proc-macro-generated error, and CARGO_INCREMENTAL=0 does not. No edit is needed: the difference is already there between an incremental and a non-incremental build. A P6 check that compares against a clean incremental build would not see it. It shows only when the reference build is non-incremental (CARGO_INCREMENTAL=0) or when diagnostic counts are compared.

*This run:* before: the incremental build prints the error 4 times, a clean build once. After: once each. Outputs: [`before`](motivating/out/162901/before.txt), [`after`](motivating/out/162901/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2026-10-01 (also 1.85.0, 1.92.0, 1.96.1), the non-incremental build prints 1 E0277 error and the incremental build prints 4 identical copies ('aborting due to 4 previous errors'). On nightly-2026-10-03 both print 1. Stable 1.70.0 prints 1 in both modes, so this form of the bug arrived later; #106571's proc-macro form dates from nightly-2023-01-03. Regression test: tests/ui/diagnostic-flags/deduplicate-diagnostics-incr.rs.

## P7: Nothing is left behind in the output directory

Leftover temporary files waste space and can be picked up by later builds.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#107001](https://github.com/rust-lang/rust/issues/107001) rustc leaves *.rcgu.o object files in the output directory when a post-monomorph | [#110107](https://github.com/rust-lang/rust/pull/110107) | 2023-04-21 | `nightly-2023-04-21` → `nightly-2023-04-22` | reproduced | — |
| [#139899](https://github.com/rust-lang/rust/issues/139899) rustdoc --test leaves rustdoctest* temporary directories behind whenever a docte | [#140706](https://github.com/rust-lang/rust/pull/140706) | 2025-05-08 | `nightly-2025-05-07` → `nightly-2025-05-09` | reproduced | — |

**#107001.** The const-prop lints (unconditional_panic, arithmetic_overflow) ran in mir_drops_elaborated_and_const_checked, which was only forced lazily, so their errors could fire after codegen had already written the CGU object files. rustc then aborted without deleting them, leaving `<crate>.<hash>.rcgu.o` / `<crate>.<crate>.<hash>-cgu.N.rcgu.o` in the output directory (target/debug/deps under cargo). PR #110107 ('Ensure mir_drops_elaborated_and_const_checked when requiring codegen') makes sure that query runs before codegen. Its description says: 'may emit errors while codegen has started, and the compiler would exit leaving object code files around. Found by @cuviper in #109731' (cuviper's comment there: 'each time I try one of these failing tests, it's leaving temporary *.rcgu.o files around'). Issue #107001 is the standalone report with this exact repro. It is still marked open, but its repro stops leaking at this PR. I bisected the nightlies myself and the boundary is exactly this merge: nightly-2023-04-21 (8bdcc62cb) leaks and nightly-2023-04-22 (fec9adcdb) is clean. The commit range between them contains #110107.

*This run:* before: a failed compile leaves `.rcgu.o` files in the output directory. After: none. Outputs: [`before`](motivating/out/107001/before.txt), [`after`](motivating/out/107001/after.txt).

*Expected, from the issue and the research:* Both toolchains fail the same way: 'error: this operation will panic at runtime ... index out of bounds: the length is 5 but the index is 9' with `#[deny(unconditional_panic)]`, exit=1. On nightly-2023-04-21 (also stable 1.70.0), `ls -A out` lists 6 leftover object files, e.g. `code.1tgaf0fuackrygys.rcgu.o code.code.56d798bc-cgu.0.rcgu.o ...`. On nightly-2023-04-22 (also stable 1.71.0 and everything since, through nightly-2026-10-06), `out` is empty. Verified locally.

**#139899.** When any doctest failed, rustdoc (or libtest) called process::exit, so the TempDir destructor never ran and the `rustdoctestXXXXXX` directory was never removed. Issue #139899 reports 6197 of them piling up in /tmp. PR #140706 ('[rustdoc] Ensure that temporary doctest folder is correctly removed even if doctests failed') adds a libtest hook that runs after all tests and cleans the folder up. It also adds the regression test tests/run-make/rustdoc/doctest/tempdir-removal, whose two input files are used verbatim below. The leftovers go to TMPDIR, not to target/, so this hits P7 only when TMPDIR points inside the build tree. It is still a real, fixed 'temp dir left behind on the failure path' bug in the toolchain that `cargo test --doc` runs. Note: this is rustdoc, not rustc.

*This run:* before: each failing doctest leaves a `rustdoctest*` directory (4 left). After: none. Outputs: [`before`](motivating/out/139899/before.txt), [`after`](motivating/out/139899/after.txt).

*Expected, from the issue and the research:* Every run exits 101 (failed doctest) on both toolchains. On nightly-2025-05-07, `ls -A tmp` shows one leftover directory per run (4 in total), e.g. `rustdoctestLP6B1A rustdoctesteGUqc8 rustdoctest0kc32h rustdoctestqVdzVs`. On nightly-2025-05-09, `tmp` is empty. Verified locally for both editions (2018 per-test and 2024 merged doctests).

## tracked: Cross-crate reads are tracked, and metadata is reused when nothing changed

Every query that reads another crate's metadata must record a dependency on that crate, or a later session reuses a stale result. The `tracked` column of the blessed list records it per query; the touch-only rebuild records whether metadata was reused.

| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |
|---|---|---|---|---|---|
| [#82920](https://github.com/rust-lang/rust/issues/82920) Miscompilation with incr. comp. | [#83074](https://github.com/rust-lang/rust/pull/83074) | 2021-03-15 | `nightly-2021-03-13` → `nightly-2021-03-17` | reproduced | — |
| [#84252](https://github.com/rust-lang/rust/issues/84252) ICE: found unstable fingerprints for has_global_allocator(): the query read untr | [#84260](https://github.com/rust-lang/rust/pull/84260) | 2021-04-17 | `nightly-2021-04-16` → `nightly-2021-04-19` | reproduced | — |
| [#89598](https://github.com/rust-lang/rust/issues/89598) VTable-related miscompilation with incremental compilation | [#89619](https://github.com/rust-lang/rust/pull/89619) | 2021-10-08 | `nightly-2021-10-07` → `nightly-2021-10-10` | reproduced | — |
| [#111295](https://github.com/rust-lang/rust/issues/111295) debugger_visualizer: edits to the visualizer script are not picked up under incr | [#111641](https://github.com/rust-lang/rust/pull/111641) | 2023-05-19 | `nightly-2023-05-18` → `nightly-2023-05-21` | reproduced | — |
| [#114669](https://github.com/rust-lang/rust/issues/114669) Metadata was never reused across incremental sessions: always re-encoded | [#114669](https://github.com/rust-lang/rust/pull/114669) | 2025-07-04 | `nightly-2025-07-03` → `nightly-2025-07-06` | reproduced | with #143247 reverted (metadata depending on a node that is never green), the touch rebuild's record catches it |

**#82920.** Bounds and predicates were sorted by DefId, which is not stable across sessions. When two trait declarations swap places, the query result changes but is still treated as green and reused, so the vtable layout and the method call sites disagree. The incremental binary calls the wrong trait method, while a clean build is correct. The original report was a rust-analyzer test suite failing with out-of-bounds panics and segfaults until `cargo clean`. Labelled I-unsound.

*This run:* before: the incremental pass-2 binary panics (`left: 2, right: 1`) while a clean build of the same source passes. After: both pass. Outputs: [`before`](motivating/out/82920/before.txt), [`after`](motivating/out/82920/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2021-03-13, the incremental pass 2 binary panics with "assertion failed: `(left == right)` left: `2`, right: `1`" (method_two was called for method_one), while a clean rpass2 build prints 'clean ok'. On nightly-2021-03-17 both passes print ok. Regression test: tests/incremental/issue-82920-predicate-order-miscompile.rs.

**#84252.** has_global_allocator was answered from the CStore (the crate loader's per-crate metadata state) without recording any dependency. PR #84260: 'This query reads from untracked global state in `CStore`'. It was fixed by making it eval_always. The repro is the regression test tests/incremental/issue-84252-global-alloc.rs: removing #[global_allocator] between sessions leaves the cached result stale, and recomputing it gives a different value. The sibling fix #83153 (extern_mod_stmt_cnum made eval_always for the same reason, issue #83126) is a related example. #83126 is still open on GitHub, so I left it out as a separate entry.

*This run:* before: the second incremental build ICEs with unstable fingerprints for `has_global_allocator`. After: both builds succeed. Outputs: [`before`](motivating/out/84252/before.txt), [`after`](motivating/out/84252/after.txt).

*Expected, from the issue and the research:* Before the fix (nightly-2021-04-16): the second build panics with "found unstable fingerprints for has_global_allocator(lib[8787]): false" (assertion left/right Fingerprint mismatch in rustc_query_system plumbing.rs) and exits non-zero. After the fix (nightly-2021-04-19): rc1=0 and rc2=0.

**#89598.** #86475 added an untracked global vtable cache to tcx. After trait methods are reordered, the object file for the main CGU is reused from the incremental cache with the old vtable layout, while mod1 is recompiled for the new layout. The incremental binary then calls method2 where a clean build calls method1, so the binaries differ (a miscompile). P-critical, I-unsound, regression-from-stable-to-beta.

*This run:* before: the incremental pass-2 binary calls the wrong method (`left: 42, right: 17`); the clean build passes. After: both pass. Outputs: [`before`](motivating/out/89598/before.txt), [`after`](motivating/out/89598/after.txt).

*Expected, from the issue and the research:* Verified locally. On nightly-2021-10-07, pass 1 prints 'pass1 ok'; the incremental pass 2 binary panics with "assertion failed: `(left == right)` left: `42`, right: `17`" because method2 was called through the stale vtable. The clean build of rpass2 runs fine. On nightly-2021-10-10 both passes print ok. Regression test: tests/incremental/reorder_vtable.rs.

**#111295.** The debugger_visualizers query result (the script contents, which are also encoded into crate metadata so downstream binaries embed upstream visualizers) did not depend on the script file. After the .py file was edited, an incremental rebuild reused the stale contents. #111641 ('Fix dependency tracking for debugger visualizers') made the query eval_always, and the fix also hashes the visualizer contents into crate_hash. The code comment says 'that content is exported into crate metadata, so any changes to it need to be reflected in the crate hash', so downstream crates that read it from metadata also see the change. The same PR fixes #111227 (an ICE from changing a natvis file with --crate-type=rlib) and #111226 (dep-info). The repro is single-crate, as in the issue. No gdb is needed: read the .debug_gdb_scripts section directly.

*This run:* before: the rebuilt binary still embeds the old visualizer script. After: the new one. Outputs: [`before`](motivating/out/111295/before.txt), [`after`](motivating/out/111295/after.txt).

*Expected, from the issue and the research:* Before the fix (nightly-2023-05-18): the rebuilt binary's .debug_gdb_scripts still contains print('hello!'), the stale script. After the fix (nightly-2023-05-21): it contains print('hello world'). Linux/ELF only; needs binutils objcopy.

**#114669.** A performance violation of 'metadata is reused (not re-encoded) when nothing it depends on changed'. Before July 2025, metadata encoding depended on the forever-red DepNode (iter_local_def_id / def_path_table read DepNodeIndex::FOREVER_RED_NODE) and was not a dep-graph task, so every incremental session re-encoded the .rmeta from scratch. #143247 (merged 2025-07-04, split out of #114669 'for perf') removed the forever-red read by depending on `analysis` instead. #114669 (merged 2025-07-04) wraps encoding in a Metadata dep-node task, saves the rmeta as a work product ('metadata') in the incremental dir, and when the node is green it hardlinks or copies the saved file instead of encoding ('can yield substantial gains (~10%)... if all the changes are in upstream crates and have no effect on it'). Observable without logs: the reused rmeta is a hardlink of the work product, so its inode stays the same across no-op sessions. Verified on both toolchains. This is a numbered PR, not an issue: no separate GitHub issue exists, so the issue field holds the PR number.

*This run:* before: an unchanged rebuild re-encodes the metadata (new inode). After: reused (same inode, hard-linked to the work product). Outputs: [`before`](motivating/out/114669/before.txt), [`after`](motivating/out/114669/after.txt).

*Expected, from the issue and the research:* Before (nightly-2025-07-03): run1 and run2 report different inodes and links=1, so the rmeta is freshly encoded on the unchanged rebuild. After (nightly-2025-07-06): links=2 (the file is hardlinked to the 'metadata' work product in the incremental dir) and run2 has the same inode as run1, so the metadata was reused rather than re-encoded. Note: after the fix, editing even a private fn body still changed the inode in my test (the Metadata node went red), so the no-op rebuild is the clean demonstration. Needs a filesystem with hardlinks; on one without them link_or_copy falls back to copying, and the inode check does not apply.

## Found by mirth (P6)

| bug | reproduce | cause | since |
|---|---|---|---|
| `Generics::param_def_id_to_index` order changes on each round trip through the incremental cache | `docs/hunt/repro.sh` (p6-generics) | an `FxHashMap` encoded in iteration order | at least 1.95 |
| a string literal encoded twice after an incremental rebuild | `docs/hunt/repro.sh` (p6-literals) | literals deduplicated when created but not when decoded | 1.90 (#116707) |
| the previous session's metadata republished after an edit that moves no span | `docs/hunt/repro.sh` (stale-source) | source map file hashes and lengths not tracked | 1.90 (#114669) |

Each reproduces on the official nightly and on stable 1.98.1, and has a draft report, a
candidate fix and a regression test in [`hunt/`](hunt).

## Not covered here

The 29 properties in [`properties.md`](properties.md) cite the bugs that suggested them,
checked against the issue text, but those bugs have not been reproduced this way.

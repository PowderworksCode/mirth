# Finding 9: a dylib fails to link with -Cno-prepopulate-passes and ThinLTO without shared generics

Facts for a report; the report itself is for a person to write (rust-lang/rust's LLM policy).

**Repro.** `docs/hunt/repro.sh`, "no-prepopulate-link":

    echo 'pub fn f(a: &mut u8, b: &mut u8) { core::mem::swap(a, b) }' > a.rs
    rustc --edition 2021 --crate-type dylib -Ccodegen-units=16 \
        -Cno-prepopulate-passes -Zshare-generics=no -Zthinlto=yes a.rs

**Expected.** `liba.so`.

**Actual.** `rust-lld: error: undefined hidden symbol` for
`<usize>::unchecked_add::precondition_check`, `core::ptr::swap_nonoverlapping::<u8>`,
`<*const ()>::is_aligned_to` and `core::ub_checks::maybe_is_nonoverlapping::runtime`.

**Conditions.** All three flags are needed; any number of codegen units from 2. A cdylib
fails the same way. A bin and a staticlib link. `-Copt-level=1` with
`-Cno-prepopulate-passes -Zshare-generics=no` (local ThinLTO is on by default there) fails
too. With `-Clto=thin` it links. Not specific to incremental compilation.

**Versions.** Stable releases with `RUSTC_BOOTSTRAP=1`: links on 1.53.0 through 1.77.0,
fails on 1.78.0 through 1.98.1 and nightly-2026-10-06. 1.78 is when these `ub_checks`
helpers appeared in `core`, so older releases may only lack a function that shows it.

**Cause, as far as followed.** With `-Csave-temps`, the defining codegen unit's copy is
`define hidden` in `thin-lto-input` and `define internal` from `thin-lto-after-internalize`
on, while the calling unit still only `declare`s it after `thin-lto-after-import`: ThinLTO
internalized a definition another module needs and the import did not happen. Why
`-Cno-prepopulate-passes` changes this was not followed further.

**How mirth found it.** The transitions walk on `fixtures/sink` (`docs/flags.md`), whose
`sink-dy` crate is a dylib, then a delta minimization of the row's 130 flags.

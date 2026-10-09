# Finding 12: rustc segfaults on `-g -Crelocation-model=rwpi` for x86_64 with a mutable static

Facts for a report; the report itself is for a person to write (rust-lang/rust's LLM policy).

**Repro.** `docs/hunt/repro.sh`, "rwpi-segfault":

    echo 'pub static mut M: u32 = 0;' > b.rs
    rustc --crate-type lib -g -Crelocation-model=rwpi b.rs

on x86_64-unknown-linux-gnu. Stable flags only.

**Expected.** An rlib, or an error that `rwpi` is not supported for this target
(`ropi`/`rwpi` are ARM relocation models; `rustc --print relocation-models` lists them for
x86_64 too).

**Actual.** `error: rustc interrupted by SIGSEGV, printing backtrace`, exit 139. The
backtrace is in LLVM: `MCStreamer::visitUsedExpr` ← `MCStreamer::emitValue` ←
`DIEValue::emitValue` ← `AsmPrinter::emitDwarfDIE` ← `DwarfDebug::endModule`, on a codegen
worker thread.

**Conditions.** `-Crelocation-model=rwpi` and `ropi-rwpi` crash; `ropi` does not. Debuginfo
is needed (`-g`). The static must be writable: `static mut`, or a static with interior
mutability (`AtomicU32`); an immutable `static` and a `thread_local!` do not crash.

**Versions.** Fine on 1.20.0 through 1.59.0 (LLVM 13); crashes on 1.60.0 (LLVM 14) through
1.98.1 and nightly-2026-10-06 (LLVM 23).

**Cause, as far as followed.** The crash is in emitting the DWARF location of the global: with
RWPI, LLVM describes a writable global's address relative to the static base, and on x86_64
the expression it emits is null. rustc passes `rwpi` to LLVM for any target. An LLVM commit
about wrong debuginfo for RWPI globals on ARM
(https://repo.hca.bsc.es/gitlab/rferrer/llvm-epi/-/commit/04dc68710ad2b30a1d3b4a2ca33005af2c9460eb)
is in the same area; whether it is the change in LLVM 14 was not checked.

**How mirth found it.** Trying the hand-picked values of `-Crelocation-model` alone on
`fixtures/sink` (`docs/flags.md`), whose `sink-core` has an `AtomicU32` static.

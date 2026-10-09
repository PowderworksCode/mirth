# Findings 13 and 14: retpoline in LLVM, with the machine outliner and the large code model

Facts for reports; the reports themselves are for a person to write (rust-lang/rust's LLM
policy). Both are in LLVM's x86 backend, reached through rustc options; LLVM has its own
tracker. No local patch: these are excluded from the flag models instead (they would need an
LLVM build).

## 13: SIGSEGV with `-Cllvm-args=-enable-machine-outliner` and retpolines

**Repro.**

    cat > a.rs <<'RS'
    pub fn f(v: &[u32]) -> u32 { v.iter().map(|x| x * 3).sum() }
    pub fn g(v: &[u32]) -> u32 { v.iter().map(|x| x * 5).sum::<u32>() + f(v) }
    pub fn h(b: Box<dyn Fn(u32) -> u32>) -> u32 { b(1) + b(2) }
    RS
    rustc --crate-type lib -Copt-level=1 -Cllvm-args=-enable-machine-outliner \
        -Zretpoline=yes -Zunstable-options -Cunsafe-allow-abi-mismatch=retpoline a.rs

**Expected.** An rlib. **Actual.** `rustc interrupted by SIGSEGV`, exit 139.

**Conditions.** Any `-Copt-level` from 1 (also `s`, `z`); without retpolines it builds. Before
`-Zretpoline` existed, `-Ctarget-feature=+retpoline-indirect-calls,+retpoline-indirect-branches`
gives the same crash.

**Versions.** 1.71.0, 1.87.0 (target features), 1.93.0, 1.98.1, nightly-2026-10-06
(`-Zretpoline`).

**Without rustc.** `llc -O1 -enable-machine-outliner` on the IR rustc emits for this file
(`docs/hunt/llvm/outliner-retpoline.ll`, from `--emit=llvm-ir`; it carries
`"target-features"="+retpoline-indirect-branches,+retpoline-indirect-calls"`) segfaults with
the toolchain's own `llc` (LLVM 23.1.3); without `-enable-machine-outliner` it compiles. So
this is LLVM's. The outliner is not on by default for x86-64, which may make it a
low-priority report there.

**The outliner fails in other combinations too** (minimized from walk rows with
`rustc/flag-min.py`, on sink):

- `-Cllvm-args=-enable-machine-outliner -Copt-level=3 -Zcf-protection=full
  -Zpatchable-function-entry=4,2 -Cdebuginfo=none`: SIGSEGV.
- `-Cllvm-args=-enable-machine-outliner -Copt-level=2 -Ccode-model=large` (with a few more
  options): `error: symbol '.L6$pb' can not be undefined in a subtraction expression`.

The models now leave the outliner out entirely (`DROP` in `rustc/flag-model.py`).

## 14: LLVM 23 emits APX `jmpabs` for retpoline tail calls under the large code model

A miscompilation, not only a link error: the program dies with SIGILL on a CPU without APX.

**Repro with rustc.**

    echo 'fn main() { let v: Vec<u32> = (0..std::env::args().count() as u32 + 3).collect(); println!("{}", v.len()); }' > m.rs
    rustc -Ccode-model=large -Crelocation-model=static -Ctarget-feature=+crt-static \
        -Zretpoline=yes -Zunstable-options -Cunsafe-allow-abi-mismatch=retpoline m.rs
    ./m

**Expected.** Prints `4`. **Actual.** `Illegal instruction` (exit 132) on this machine (an
x86-64 CPU with AVX-512, no APX). The binary contains 4 `jmpabs` instructions (`d5 00 a1`
followed by a 64-bit address), which are APX instructions; nothing enabled APX. Every program
that allocates is affected, since the allocator shim's `__rust_alloc` tail-calls
`__rdl_alloc`. In a dylib (PIC) the same instruction shows up as the link error first seen:
`relocation R_X86_64_64 cannot be used against symbol '__llvm_retpoline_r11'`.

**Repro without rustc** (`docs/hunt/llvm/jmpabs-retpoline-large.ll`, 13 lines):

    target triple = "x86_64-unknown-linux-gnu"
    declare ptr @callee(i64)
    define ptr @f(i64 %x) #0 {
      %r = tail call ptr @callee(i64 %x)
      ret ptr %r
    }
    attributes #0 = { "target-features"="+retpoline-indirect-calls,+retpoline-indirect-branches" }
    !llvm.module.flags = !{!0}
    !0 = !{i32 1, !"Code Model", i32 4}

`llc` (LLVM 23.1.3, from nightly-2026-10-06) gives

    movabsq $callee, %r11
    jmpabs  $__llvm_retpoline_r11           # TAILCALL

**Versions.** With the target features (`-Zretpoline` did not exist in older releases),
binaries run on 1.78.0 (LLVM 18.1.2), 1.87.0, 1.89.0, 1.90.0 (LLVM 20.1.8), 1.91.0, 1.93.0
(LLVM 21.1.8), 1.98.1 (LLVM 22.1.8) and nightly-2026-07-18 (LLVM 22.1.8); SIGILL on
nightly-2026-10-06 (LLVM 23.1.3). A regression in LLVM 23: the large-code-model tail call to
the retpoline thunk is lowered to `jmpabs` without checking that APX is available.

**How mirth found them.** The pairwise walk over all options with the sample values
(`docs/flags.md`), then `rustc/flag-min.py` on the rows that failed.

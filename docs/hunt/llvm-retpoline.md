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

**The outliner fails in other combinations too** (minimized from walk rows with
`rustc/flag-min.py`, on sink):

- `-Cllvm-args=-enable-machine-outliner -Copt-level=3 -Zcf-protection=full
  -Zpatchable-function-entry=4,2 -Cdebuginfo=none`: SIGSEGV.
- `-Cllvm-args=-enable-machine-outliner -Copt-level=2 -Ccode-model=large` (with a few more
  options): `error: symbol '.L6$pb' can not be undefined in a subtraction expression`.

The models now leave the outliner out entirely (`DROP` in `rustc/flag-model.py`).

## 14: `-Ccode-model=large` with retpolines cannot link a dylib

**Repro.**

    echo 'pub fn h(b: &dyn Fn(u32) -> u32) -> u32 { b(1) }' > c.rs
    rustc --crate-type dylib -Ccode-model=large -Zretpoline=yes \
        -Zunstable-options -Cunsafe-allow-abi-mismatch=retpoline c.rs

**Expected.** `libc.so`. **Actual.** `rust-lld: error: relocation R_X86_64_64 cannot be used
against symbol '__llvm_retpoline_r11'; recompile with -fPIC`: the call to the retpoline thunk
is emitted as an absolute address in position-independent code.

**How mirth found them.** The pairwise walk over all options with the sample values
(`docs/flags.md`), then `rustc/flag-min.py` on the rows that failed.

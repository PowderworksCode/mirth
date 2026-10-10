#!/usr/bin/env bash
# Finding 20: under the RISC-V (and LoongArch) hard-float calling convention, rustc passes a
# `repr(C)` struct of one float and one pointer in a float register plus an integer register.
# The psABI's float+integer rule covers integers only, not pointers; clang (and GCC) pass such a
# struct in two integer registers. A Rust function and a C caller disagree about where the
# argument is.
#
# Found by mirth's ABI differential (`mirth-lab abi-diff`), which generates random C signatures
# and compares rustc's LLVM IR against clang's for the same signature. Not found in a public
# crate. Present since at least 1.80.0.
#
# Needs: rustup, cargo, clang with the RISC-V backend. No riscv hardware or emulator.
set -euo pipefail
TOOLCHAIN=${TOOLCHAIN:-stable}
TARGET=riscv64gc-unknown-linux-gnu
rustup target add --toolchain "$TOOLCHAIN" "$TARGET" >/dev/null

dir=$(mktemp -d)
cd "$dir"
cargo +"$TOOLCHAIN" init -q --lib --edition 2021 --name fp

cat > src/lib.rs <<'EOF'
#![no_std]

#[repr(C)]
pub struct FP {
    pub f: f32,
    pub p: *mut u8,
}

/// Control: a float and a real integer, which the float+integer rule does cover.
#[repr(C)]
pub struct FI {
    pub f: f32,
    pub i: i64,
}

#[no_mangle]
pub extern "C" fn take_fp(a: FP) -> f32 {
    a.f
}

#[no_mangle]
pub extern "C" fn take_fi(a: FI) -> f32 {
    a.f
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
EOF

cat > fp.c <<'EOF'
struct FP { float f; void *p; };
struct FI { float f; long i; };
float take_fp(struct FP a) { return a.f; }
float take_fi(struct FI a) { return a.f; }
EOF

cargo +"$TOOLCHAIN" rustc -q --target "$TARGET" -- --emit=llvm-ir -Cpanic=abort
rust_ll=$(ls target/$TARGET/debug/deps/fp-*.ll)
clang --target=riscv64-unknown-linux-gnu -march=rv64gc -mabi=lp64d -O2 -S -emit-llvm -o fp.ll fp.c

for f in take_fp take_fi; do
  echo "== $f"
  echo "  rustc: $(grep -E "^define .*@$f\(" "$rust_ll" | grep -oE "@$f\(.*\)" | head -1)"
  echo "  clang: $(grep -E "^define .*@$f\(" fp.ll | grep -oE "@$f\(.*\)" | head -1)"
done

# The bug: for take_fp clang uses `[2 x i64]` (two integer registers, a0/a1) while rustc uses
# a float and an integer (fa0/a0). take_fi is the control: both use float + i64.
r_fp=$(grep -E '^define .*@take_fp\(' "$rust_ll")
c_fp=$(grep -E '^define .*@take_fp\(' fp.ll)
if [[ "$c_fp" == *"[2 x i64]"* ]] && [[ "$r_fp" == *"float"* ]]; then
  echo "REPRODUCED: clang passes struct { float; void* } as [2 x i64], rustc as a float and an integer."
else
  echo "NOT REPRODUCED"
fi
echo "(work directory: $dir)"

#!/usr/bin/env bash
# Finding 19: on riscv64 (and loongarch64), rustc does not sign-extend an `i32` argument of an
# `extern "C"` call when it is passed on the stack (more integer arguments than the 8 argument
# registers). The RISC-V psABI requires it, clang does it, and a C callee may rely on it.
#
# Found by mirth's ABI differential (`mirth-lab abi-diff`), which generates random C signatures
# and compares rustc's LLVM IR against clang's for the same signature. Not found in a public
# crate. Present since at least 1.80.0.
#
# Needs: rustup, cargo, clang (any recent version with the RISC-V backend). No riscv hardware or
# emulator: the check compares the two compilers' LLVM IR and rustc's assembly.
set -euo pipefail
TOOLCHAIN=${TOOLCHAIN:-stable}
TARGET=riscv64gc-unknown-linux-gnu
rustup target add --toolchain "$TOOLCHAIN" "$TARGET" >/dev/null

dir=$(mktemp -d)
cd "$dir"
cargo +"$TOOLCHAIN" init -q --lib --edition 2021 --name caller

cat > src/lib.rs <<'EOF'
#![no_std]

extern "C" {
    fn callee(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64, i9: i32) -> i64;
}

/// The ninth argument does not fit in a0-a7, so it goes on the stack.
#[no_mangle]
pub unsafe extern "C" fn caller(x: i64) -> i64 {
    callee(0, 0, 0, 0, 0, 0, 0, 0, x as i32)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
EOF

cat > callee.c <<'EOF'
long callee(long a, long b, long c, long d, long e, long f, long g, long h, int i9) { return i9; }
EOF

# Rust side: LLVM IR and assembly of the caller.
cargo +"$TOOLCHAIN" rustc -q --release --target "$TARGET" -- --emit=llvm-ir,asm -Cpanic=abort
rust_ll=$(ls target/$TARGET/release/deps/caller-*.ll)
rust_s=$(ls target/$TARGET/release/deps/caller-*.s)

# C side: the same prototype through clang, for the target's default ABI (lp64d).
clang --target=riscv64-unknown-linux-gnu -march=rv64gc -mabi=lp64d -O2 -S -emit-llvm -o callee.ll callee.c

echo "== rustc's declaration of callee (ninth parameter is the i32):"
grep -E '^declare .*@callee' "$rust_ll"
echo "== clang's definition of callee:"
grep -E '^define .*@callee' callee.ll
echo "== rustc's caller (assembly):"
sed -n '/^caller:/,/^\.Lfunc_end/p' "$rust_s" | grep -vE '^\s*\.(cfi|p2align|type|size)'

# The bug: clang marks the ninth parameter `signext` (the callee may assume a sign-extended
# 64-bit stack slot); rustc does not, and its caller stores the full 64-bit register
# (`sd`) without a `sext.w` first. caller(0x1_0000_0005) hands C an `int` holding 0x1_0000_0005.
c_ext=$(grep -E '^define .*@callee' callee.ll | grep -oE 'i32 [a-z ]*signext' || true)
r_ext=$(grep -E '^declare .*@callee' "$rust_ll" | grep -oE 'i32 [a-z ]*signext' || true)
if [ -n "$c_ext" ] && [ -z "$r_ext" ]; then
  echo "REPRODUCED: clang passes the stack i32 as 'signext', rustc passes it without extension."
else
  echo "NOT REPRODUCED: clang '$c_ext', rustc '$r_ext'."
fi
echo "(work directory: $dir)"

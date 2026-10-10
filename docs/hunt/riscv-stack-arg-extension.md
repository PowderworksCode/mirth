# riscv64 and loongarch64: integer arguments passed on the stack are not sign-extended

Facts for finding 19. Found by the ABI differential ([`checks.md`](../checks.md), check 14:
`mirth-lab abi-diff`, rustc's `extern "C"` lowering against clang's for random C signatures).

## What happens

On riscv64 and loongarch64, when an `extern "C"` call has more integer arguments than argument
registers (eight), the rest go on the stack. rustc sign-extends an `i32` (or narrower integer)
to 64 bits only when it is passed in a register. On the stack it stores the whole 64-bit
register, upper bits included. clang marks every such parameter `signext`, and a C callee
compiled by clang reads the stack slot as an already-extended 64-bit value. A Rust caller can
therefore hand C a value outside the `int` range.

## Reproduction

```c
// callee.c
long callee(long a, long b, long c, long d, long e, long f, long g, long h, int i9) { return i9; }
```

```rust
// caller.rs (#![no_core] with minicore, or any crate for the target)
extern "C" { fn callee(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64, i9: i32) -> i64; }
#[no_mangle] pub unsafe extern "C" fn caller(x: i64) -> i64 { callee(0, 0, 0, 0, 0, 0, 0, 0, x as i32) }
```

`rustc --target riscv64gc-unknown-linux-gnu -Copt-level=2 --emit=asm` (pinned nightly):

```
caller:
	mv	t0, a0          # x, all 64 bits
	li	a0, 0           # ... a1-a7 = 0
	sd	t0, 0(sp)       # the ninth argument: stored without sign extension
	call	callee
```

`clang --target=riscv64-unknown-linux-gnu -O2 -S callee.c`:

```
callee:
	ld	a0, 0(sp)       # read as a sign-extended 64-bit value and returned as is
	ret
```

`caller(0x1_0000_0005)` returns `0x1_0000_0005`. The C function's `int` parameter holds a value
an `int` cannot have, and C code is entitled to rely on the extension.

The LLVM IR shows the cause. clang declares `i32 noundef signext` for both `int` parameters of
`long callee(long ×8, int, int)`. rustc declares `i32 noundef` with no `signext` for the
parameters that do not fit in registers. loongarch64 is the same (`st.d $a0, $sp, 0` in the
caller, `ld.d $a0, $sp, 0` in the callee).

## Expected

The RISC-V psABI (riscv-cc.adoc, integer calling convention) says: "When passed in registers
or on the stack, integer scalars narrower than XLEN bits are widened according to the sign of
their type up to 32 bits, then sign-extended to XLEN bits." clang does this for every argument.
The LoongArch psABI has the same rule for the stack.

## Where

`compiler/rustc_target/src/callconv/riscv.rs`, end of `classify_arg`:

```rust
    // "When passed in registers, scalars narrower than XLEN bits are widened
    // according to the sign of their type up to 32 bits, then sign-extended to
    // XLEN bits."
    if *avail_gprs >= 1 {
        extend_integer_width(arg, xlen);
        *avail_gprs -= 1;
    }
```

The quoted text is the older wording; the extension is skipped once the registers run out.
`callconv/loongarch.rs` has the same structure.

## Versions

The IR lacks `signext` on stack-passed `i32` parameters with 1.80.0, 1.90.0, 1.98.0 and
nightly-2026-10-06. Older toolchains did not build the `no_core` test file and were not checked.

## Scope

- Rust calling C (or any clang/GCC-compiled callee) with more than eight integer-class arguments,
  where a narrow integer lands on the stack. riscv32 has the same rule for `i8`/`i16` widened to
  32 bits; not yet checked.
- C calling Rust is not affected the same way: a Rust callee without `signext` re-extends the
  value itself.

## Local stopgap

None yet: it does not affect mirth's incremental checks. The ABI differential records it as
known.

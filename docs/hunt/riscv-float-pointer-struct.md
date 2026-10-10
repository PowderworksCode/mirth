# RISC-V and LoongArch: a struct of a float and a pointer goes in the wrong registers

Facts for finding 20. Found by the ABI differential ([`checks.md`](../checks.md), check 14:
`mirth-lab abi-diff`).

## What happens

Under the hardware floating-point calling conventions (riscv64 lp64d, riscv32 ilp32d,
loongarch64 lp64d), a struct with one floating-point member and one *integer* member is passed
in one floating-point register and one integer register. rustc applies that rule when the
second member is a raw pointer. clang does not: it passes the struct by the integer convention
(two integer registers, or by reference if it is too large). A C function and a Rust function
declared with the same struct therefore disagree about where the arguments are.

## Reproduction

```c
struct FP { float f; void *p; };
void take_fp(struct FP a);
```

```rust
#[repr(C)] pub struct FP { f: f32, p: *mut u8 }
#[no_mangle] pub extern "C" fn take_fp(a: FP) { /* ... */ }
```

LLVM IR of the definition, `--target riscv64gc-unknown-linux-gnu` / `riscv64-unknown-linux-gnu -mabi=lp64d`:

| signature | clang 21 | rustc (pinned nightly) |
|---|---|---|
| `void take_fp(struct { float; void*; })` | `[2 x i64]` (a0, a1) | `{ float, i64 }` (fa0, a0) |
| `void take_dp(struct { double; void*; })` | `[2 x i64]` | `{ double, i64 }` |
| `void take_pf(struct { void*; float; })` | `[2 x i64]` | `{ i64, float }` |
| `struct { float; void*; } ret_fp(void)` | `[2 x i64]` | `{ float, i64 }` |
| `void take_fi(struct { float; long; })` (control) | `float, i64` | `{ float, i64 }`: same registers |

loongarch64 gives the same table. On riscv32 (ilp32d), `struct { double; void*; }` is passed by
reference by clang (larger than two XLEN words, and not eligible for the FP convention) and as
`{ double, i32 }` by rustc.

## Expected

The RISC-V psABI (hardware floating-point calling convention): "A struct containing one
floating-point real and one integer (or bitfield), in either order, is passed in a
floating-point register and an integer register…". A pointer is not an integer type. clang's
`detectFPCCEligibleStruct` takes only integral and enumeration types. GCC's
`riscv_flatten_aggregate_field` also tests for an integral type; that is from reading its
source, and its output was not checked here (no riscv GCC on this host). The LoongArch psABI
uses the same rule.

## Where

`compiler/rustc_target/src/callconv/riscv.rs`, `should_use_fp_conv_helper`:

```rust
        BackendRepr::Scalar(scalar) => match scalar.primitive() {
            Primitive::Int(..) | Primitive::Pointer(_) => {
```

`callconv/loongarch.rs` line 47 has the same arm.

## Versions

Same IR (`{ float, i64 }`) with 1.80.0, 1.90.0, 1.98.0 and nightly-2026-10-06.

## Scope

Any `extern "C"` function, in either direction, whose signature has a `repr(C)` struct (by
value, argument or return) made of one float or double and one pointer (or a struct nesting
them), on riscv64gc-unknown-linux-gnu (tier 2 with host tools), the other hard-float RISC-V
targets, and loongarch64 hard-float targets. Soft-float targets (`*-softfloat`, `riscv*imac`)
are not affected: the FP convention does not apply there.

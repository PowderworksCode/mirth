# mips64: narrow integer arguments are no longer sign- or zero-extended

Facts for finding 44. Found by the ABI differential's assembly-level mode ([`checks.md`](../checks.md),
check 14: `mirth-lab abi-diff --asm --all`).

## What happens

On the n64 targets (mips64, mips64el, mipsisa64r6, mipsisa64r6el `-unknown-linux-gnuabi64`/`-muslabi64`,
mips64-openwrt-linux-musl), rustc declares `extern "C"` integer parameters narrower than 64 bits
without `signext`/`zeroext`, in registers and on the stack alike, and a Rust caller passes them with
whatever is in the upper bits. The n64 convention extends them: 32-bit integers are sign-extended
to 64 bits whatever their signedness, narrower ones by their sign. clang declares
`signext`/`zeroext` on every such parameter, and a C callee compiled by clang or GCC may rely on
it.

## Reproduction

```rust
// any no_core crate for --target mips64-unknown-linux-gnuabi64
extern "C" { fn callee32(x: i32); fn callee8(x: i8); }
#[no_mangle] pub unsafe extern "C" fn caller32(x: i64) { callee32(x as i32) }
#[no_mangle] pub unsafe extern "C" fn caller8(x: i64) { callee8(x as i8) }
```

`rustc -Copt-level=2 -Crelocation-model=static --emit=asm`:

| | nightly-2026-07-18 | nightly-2026-10-06 |
|---|---|---|
| `declare void @callee32` | `(i32 signext)` | `(i32)` |
| `declare void @callee8` | `(i8 signext)` | `(i8)` |
| `caller32`, delay slot of `jal callee32` | `sll $4, $4, 0` | `nop` |
| `caller8`, delay slot of `jal callee8` | `seb $4, $4` | `nop` |

Rust callees are unaffected: without the attribute they re-extend (`take32: sll $2, $4, 0`).
Returns keep their extension (the return path was not changed).

## Where

PR #163653 ("callconv: mips64: Match GCC for alignment of 16-byte scalars", merged 2026-10-04,
fixing #161679) added to `classify_arg` in `compiler/rustc_target/src/callconv/mips64.rs`, after
`extend_integer_width_mips(arg, 64)`:

```rust
        if let BackendRepr::Scalar(scalar) = arg.layout.backend_repr {
            ...
            arg.cast_to_and_pad_i32(CastTarget::from(Reg { kind, size }), pad_i32);
        }
```

Every scalar now becomes `PassMode::Cast`, also when `pad_i32` is 0, and the cast carries its own
(empty) attributes, so the extension set just before is dropped.

## Versions

`signext` present with 1.80.0, 1.90.0, 1.98.0, nightly-2025-04-11, nightly-2025-12-20 and
nightly-2026-07-18; absent with nightly-2026-10-06 (the first nightly after the merge was not
installed here). The commit range between nightly-2026-07-18 and nightly-2026-10-06 has one change
to `mips64.rs` that alters scalar lowering, 22067c76adf (#163653). Not confirmed by reverting it.

## Scope

Rust calling C (or any non-Rust callee) on the tier 3 n64 targets, with an `i32`, `u32`, `i16`,
`u16`, `i8`, `u8` or `bool` argument. Searching rust-lang/rust for "mips64 signext" and the PR
number found nothing.

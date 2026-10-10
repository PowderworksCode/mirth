# thumbv7a Windows: homogeneous float aggregates are not passed in VFP registers

Facts for finding 45. Found by the ABI differential's assembly-level mode ([`checks.md`](../checks.md),
check 14).

## What happens

On thumbv7a-pc-windows-msvc and thumbv7a-uwp-windows-msvc, scalar `float`/`double` arguments go in
VFP registers on both sides (the target's LLVM float ABI is hard). A struct of one to four floats
or doubles, or a union of one float type, is passed and returned by clang in VFP registers (the
AAPCS VFP rules for homogeneous aggregates; clang marks these functions `arm_aapcs_vfpcc`). rustc
passes it in core registers and returns it through memory.

```c
struct Pair { float a, b; };
void take_pair(struct Pair p);
struct Pair make_pair(float a, float b);
```

Placement after instruction selection (`-O1`):

| | clang `--target=thumbv7a-pc-windows-msvc` | rustc (pinned nightly) |
|---|---|---|
| `make_pair` arguments / return | s0, s1 / s0, s1 | sret pointer in r0, then s0, s1 / memory |
| `pass_pair(struct Pair)` argument | s0, s1 | r0, r1 |

rustc's IR: `define void @make_pair(ptr sret([8 x i8]) align 4, float, float)` and
`define void @pass_pair([2 x i32])`, the same with 1.80.0, 1.90.0, 1.98.0, nightly-2026-07-18 and
nightly-2026-10-06.

A second difference on the same targets, for aggregates with an `aligned(N)` attribute above 8:
rustc chooses 4- or 8-byte units from the natural alignment (`unadjusted_abi_align`), clang from
the declared alignment (on Linux targets clang uses the natural one, like rustc). Which one MSVC
uses was not checked; no MSVC here.

## Where

`compiler/rustc_target/src/callconv/arm.rs`, `compute_abi_info`: the VFP rules for homogeneous
aggregates apply when `abi_kind` is `AapcsVfp`, which is chosen only when
`cx.target_spec().cfg_abi == CfgAbi::EabiHf`. These targets have `llvm-floatabi: hard` and no
`eabihf` ABI, so they get the base AAPCS rules for aggregates while LLVM uses the VFP convention
for scalars.

## Expected

Microsoft's ARM32 ABI overview says non-variadic functions follow the ARM parameter-passing rules
including the VFP extensions (from the documentation; not checked against MSVC here). clang
implements that.

## Scope

Tier 3 targets; any `extern "C"` function, either direction, with a homogeneous float aggregate
argument or return. No rust-lang/rust issue found ("thumbv7a windows").

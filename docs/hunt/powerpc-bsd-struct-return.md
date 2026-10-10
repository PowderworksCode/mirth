# 32-bit PowerPC BSDs: small structs are returned through memory, clang returns them in registers

Facts for finding 42. Found by the ABI differential's assembly-level mode ([`checks.md`](../checks.md),
check 14).

## What happens

On powerpc-unknown-freebsd, -netbsd, -openbsd and -helenos, clang returns an aggregate of up to 8
bytes in r3 (and r4). rustc returns every aggregate through a hidden pointer, as on Linux.

```c
struct Small { short a; };
struct Small make_small(short a);
```

| | registers in | return |
|---|---|---|
| clang `--target=powerpc-unknown-freebsd13.0` | r3 (`a`) | r3 |
| clang `--target=powerpc-unknown-linux-gnu` | r3 (sret), r4 (`a`) | memory |
| rustc `--target powerpc-unknown-freebsd` (1.80.0 … nightly-2026-10-06) | r3 (sret), r4 (`a`) | memory |

In the random signatures (3 seeds × 300) all 300 placement differences on these four targets are
this case, and clang and rustc agree on powerpc-unknown-linux-gnu.

## Where

`compiler/rustc_target/src/callconv/powerpc.rs`:

```rust
fn classify_ret<Ty>(ret: &mut ArgAbi<'_, Ty>) {
    if ret.layout.is_aggregate() {
        ret.make_indirect();
```

with no per-OS distinction. clang returns small aggregates in registers for 32-bit PowerPC ELF
targets other than Linux (its default when neither `-msvr4-struct-return` nor
`-maix-struct-return` is given); observed above, the source was not read here.

## Expected

The C compiler of the platform decides. FreeBSD's system compiler on powerpc is clang (since
FreeBSD 13). GCC's defaults on NetBSD and OpenBSD were not checked; no PowerPC GCC here.

## Scope

Tier 3 targets; Rust calling C or C calling Rust with an aggregate return of at most 8 bytes. No
rust-lang/rust issue found ("powerpc freebsd struct return").

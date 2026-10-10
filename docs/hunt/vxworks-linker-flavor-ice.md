# VxWorks targets: `-Clinker-flavor=ld.lld` panics in rustc's linker setup

Facts for finding 59. Found by the cross-target link check (`mirth-lab xlink`,
[`checks.md`](../checks.md) check 5), which links a small `no_std` probe for every target with
`rust-lld` when the target's default linker is not installed.

## What happens

For every `*-wrs-vxworks` target (aarch64, armv7 eabihf, i686, powerpc, powerpc-spe, powerpc64,
riscv32, riscv64, x86_64), linking with `-Clinker=rust-lld -Clinker-flavor=ld.lld` panics:

```
thread 'rustc' panicked at compiler/rustc_codegen_ssa/src/back/linker.rs:225:5:
assertion failed: l.is_cc()
```

Without `-Clinker-flavor`, the same build reports the expected "linker `wr-c++` not found". The
VxWorks linker setup assumes a C-compiler-style linker flavor; a stable command-line option
reaches the assertion instead of an error.

## Reproduction

With the probe crate (`rustc/xlink-probe`) or any `no_std` binary:

```sh
RUSTFLAGS="-Clinker=rust-lld -Clinker-flavor=ld.lld -Clink-arg=--entry=probe_entry" \
  cargo +nightly-2026-10-06 build --release -Zbuild-std=core,alloc \
  -Zbuild-std-features=compiler-builtins-mem --target x86_64-wrs-vxworks
```

## Scope

Tier 3 targets, reached only with `-Zbuild-std` (no prebuilt std) and an explicit
`-Clinker-flavor` the target does not expect. Low severity: an ICE where an error belongs. No
rust-lang/rust issue found (search for `is_cc` and vxworks, 2026-10-10). Same result in the
earlier Python xlink sweep (2026-10-09).

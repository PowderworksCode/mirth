# Reproduction scripts

Each script builds a minimal Cargo project in a temporary directory, runs the exact commands
that show the bug, checks the output, and ends with `REPRODUCED` or `NOT REPRODUCED`. They need
only rustup and cargo (plus clang for the ABI ones, GNU time for 32) and install the toolchains
or targets they use. `TOOLCHAIN=1.80.0 ./19-…sh` runs one against another release.

| script | finding | what it shows | severity |
|---|---|---|---|
| [`19-riscv-stack-arg-extension.sh`](19-riscv-stack-arg-extension.sh) | 19 | riscv64: an `i32` passed on the stack to an `extern "C"` function is not sign-extended (clang: `signext`) | high (stable, tier 2, silent wrong values across FFI) |
| [`20-riscv-float-pointer-struct.sh`](20-riscv-float-pointer-struct.sh) | 20 | riscv64: `struct { float; void* }` passed in a float and an integer register; clang uses two integer registers | high (stable, tier 2, silent wrong values across FFI) |
| [`33-rustdoc-empty-nested-use.sh`](33-rustdoc-empty-nested-use.sh) | 33 | `cargo doc` panics on `use {{}};`, which `cargo build` accepts; nightly regression since 2026-09-26 | medium |
| [`32-iter-chain-solver-regression.sh`](32-iter-chain-solver-regression.sh) | 32 | rejecting an iterator chain over the recursion limit takes ~8× the CPU and gives 75 errors instead of one under the new solver (since nightly-2026-08-04) | low |
| [`42-dead-code-used-trait.sh`](42-dead-code-used-trait.sh) | 42 | `dead_code` calls two traits "never used"; deleting them breaks the build (stable since at least 1.80) | low |

Run on this machine (2026-10-10): all five print `REPRODUCED`; 19, 20 and 42 also on 1.80.0.

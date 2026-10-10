# m68k: `repr(C)` structs with `i64` or `f64` are laid out unlike GCC's

Facts for finding 47. Found by the ABI differential's assembly-level mode ([`checks.md`](../checks.md),
check 14): every placement difference on m68k (480 functions over 3 seeds) involves a struct
containing `long long` or `double`.

## What happens

| alignment | `int`/`i32` | `long long`/`i64` | `double`/`f64` | `struct { _Bool; long long; unsigned long long; }` size |
|---|---:|---:|---:|---:|
| rustc (pinned nightly), `m68k-unknown-linux-gnu` | 2 | 4 | 8 | 20 |
| clang 21, `--target=m68k-unknown-linux-gnu` | 2 | 8 | 8 | 24 |
| GCC's documented default (`-mno-align-int`) | 2 | 2 | 2 | 18 |

rustc's values come from its data layout `E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16`,
which does not mention `i64` or `f64`, so LLVM's defaults apply (`i64:32:64`, `f64:64:64`).
clang's data layout string is the same; its `long long` and `double` alignments come from clang's
target description.

GCC's m68k options documentation: `-malign-int` aligns int, long, long long, float, double and long
double on a 32-bit boundary and `-mno-align-int` on a 16-bit boundary, and with `-malign-int` "GCC
aligns structures containing the above types differently than most published application binary
interface specifications for the m68k". The table's GCC row is from that documentation; no m68k
GCC here.

## Scope

Tier 3 targets m68k-unknown-linux-gnu and m68k-unknown-none-elf: any `repr(C)` type shared with C
that contains an `i64`, `u64` or `f64` (layout and size differ, so passing it by value or through
a pointer disagrees). Related: rust-lang/rust#117252 (open, "Wrong alignment for m68k pointer
types"), which is about pointers and `usize` (rustc 2, the m680x0 ABI document 4); its thread
mentions plans to change the default alignment on Debian and Gentoo m68k.

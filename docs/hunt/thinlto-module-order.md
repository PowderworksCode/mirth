# Finding 10: ThinLTO input order follows codegen completion

Facts for a report; the report itself is for a person to write (rust-lang/rust's LLM policy).

**Repro.** `docs/hunt/repro.sh`, "thinlto-order": a two-crate binary (an upstream rlib with a
const-generic method and an `#[inline(always)]` function) built 30 times from clean with
`-g -Clto=thin -Cincremental=<fresh dir>`. That is Cargo's dev profile with `lto = "thin"`.

**Expected.** The same object files and binary every time.

**Actual.** Two distinct sets of object files, roughly one build in five. With rust-lld (the
default linker on x86_64-unknown-linux-gnu since 1.90) the binary differs too: 8 of 30
builds, nightly-2026-10-06. Linked with BFD (`-Zunstable-options -Clinker-features=-lld`),
the binary is the same.

**What differs.** `__rustc_debug_gdb_scripts_section__`, which every codegen unit with
debuginfo defines as `linkonce_odr`, ends up kept in a different codegen unit.

**Conditions.**

| | objects | binary |
|---|---|---|
| `-g -Clto=thin -Cincremental` | differ | differ (lld) |
| same, `--jobs-backend=1 -Zunstable-options` | same (30 of 30) | same |
| same without `-g` | same | same |
| same without `-Cincremental` | same | same |
| `-g -Clto=thin -Cincremental -Copt-level=1` | same | same |
| `-g -Clto=fat -Cincremental` | same | same |
| `-g -Zthinlto=yes -Cincremental` (local ThinLTO at opt-level 0) | differ | differ |

**Versions.** Objects differ on 1.60.0, 1.71.0, 1.78.0, 1.87.0, 1.89.0 and the nightly. The
binary differs from 1.90.0 on (rust-lld).

**Cause.** `rustc_codegen_ssa/src/back/write.rs` pushes each module onto `needs_thin_lto`
as its codegen finishes (`ThinLtoInput::Red`) and never sorts the list. In
`rustc_llvm/llvm-wrapper/PassWrapper.cpp`, the prevailing copy of a linkonce symbol is
`getFirstDefinitionForLinker` of the combined index, which follows that order, so the
module that keeps the definition depends on thread timing. One backend job removes the
difference.

**How mirth found it.** The transitions walk (`docs/flags.md`) reported a rebuild differing
from a clean build in two rows; repeating clean builds showed clean builds differ too, and a
delta minimization of the flags left `-Zshare-generics=no -Zthinlto=yes`.

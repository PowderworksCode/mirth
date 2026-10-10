# Looking for new bugs

The edits and the replayed regressions test whether mirth catches bugs that
someone put in. This tests whether it finds bugs nobody knew were there: the
unmodified pinned compiler (`ea137335b`), a wider fixture, and more
incremental edits.

`fixtures/wide` has a proc-macro crate (a derive and an attribute macro), a
library with a build script that generates code, built-in derives, const
generics, a generic associated type, `impl Trait` and `async fn` in traits,
and an exported `macro_rules!`, used by a second library and a binary.
`fixtures/wide/edits` has ten changes: a private body, a new enum variant,
a doc comment, spans only, two items swapped, the derive's output, the
build script's output, a const generic argument, a macro's expansion, and
a new private function.

`rustc/hunt.sh <fixture>` builds the fixture eight times with
`-Zthreads=8` and compares the `.rmeta` files (P5). Then, for each edit, it
compares an incremental rebuild after the edit with a clean build of the
edited source (P6), with `-Zincremental-verify-ich`, single-threaded and
with `-Zthreads=8`. `rustc/check.sh wide` runs the ordinary checks.

## Findings

| | what | status |
|---|---|---|
| 1 | incremental rebuilds encode `Generics::param_def_id_to_index` in a different order from clean builds | **looks new**; root cause found; fix and regression test written |
| 2 | incremental rebuilds encode a string literal twice where clean builds encode it once | **looks new**; root cause found, regression from #116707 (1.90); fix and regression test written |
| 3 | with `-Zthreads=8`, two traits with `-> impl Trait` methods give different metadata from run to run | known: [#162202](https://github.com/rust-lang/rust/issues/162202); a testing workaround, [`hunt/threads-def-order-stopgap.patch`](hunt/threads-def-order-stopgap.patch), makes the order deterministic |
| 4 | incremental rebuilds republish the previous session's metadata when an edit moves no span, so its source map describes old files | **looks new**; found later by the fuzzer and the history replay ([`scale.md`](scale.md)); root cause found, regression from #114669 (1.90); fix and regression test written |
| 5 | six untracked options change results incremental compilation reuses; with `-Zno-leak-check`, a rebuild accepts a program a clean build rejects | **looks new**; the first three found by a query written from a closed bug and an option audit ([`ur-queries.md`](ur-queries.md)), `-Zno-leak-check`, `-C extra-filename` and `-Zfuture-incompat-test` by reporting untracked reads ([`untracked-reads.md`](untracked-reads.md)); report drafted |
| 6 | reused object code keeps the previous checksum of an edited source file in its debuginfo, and with `-Zembed-source` the previous file; with optimizations, ThinLTO symbol names then differ from a clean build | **looks new**; found by the fuzzer at `-Copt-level=2`; root cause found, since 1.44 (#69718); report drafted and a regression test (`hunt/tests/incr-debuginfo-embedded-source`, failing: no fix) |
| 7 | warnings from inline assembly are not shown again when an incremental rebuild reuses the codegen unit | **looks new**; found while checking reused codegen units ([`shadow-mode.md`](shadow-mode.md)); on 1.60.0 through the nightly; report drafted ([draft](hunt/issue-asm-warnings-reused-cgu.md)) and a regression test (`hunt/tests/incr-asm-warning-reused`, failing: no fix) |
| 8 | with `-Zmir-opt-level=3`, an incremental rebuild encodes an allocation from inlined `core` MIR twice where a clean build encodes it once | **looks new**; found by the fuzzer at `-Zmir-opt-level=4` and named by the reuse check, reduced to one line; on 1.60.0 through the nightly; report drafted ([draft](hunt/issue-inlined-alloc-identity.md)), no fix; local stopgap since 2026-10-09: the experimental patch ([patch](hunt/upstream-alloc-reference.patch)), applied after the fuzzer under `-Zmir-opt-level=16` kept hitting it; also within one crate (a constant from `Vec::new()` inlined into another body), stopgapped by giving equal immutable memory one metadata index ([patch](hunt/alloc-canonical-metadata-stopgap.patch)) |
| 9 | a dylib fails to link (undefined hidden symbols) with `-Cno-prepopulate-passes -Zshare-generics=no` and local ThinLTO | **looks new**, not incremental, unstable flags only; found by the flag transitions walk ([`flags.md`](flags.md)); since 1.78; [facts](hunt/no-prepopulate-link.md), local stopgap: no local ThinLTO with `-Cno-prepopulate-passes` ([patch](hunt/no-prepopulate-thinlto-stopgap.patch)) |
| 10 | with `-g -Clto=thin` and incremental compilation, clean builds give different object files from run to run, and since 1.90 (rust-lld) different binaries | **looks new**; found by the flag transitions walk ([`flags.md`](flags.md)), first as a rebuild differing from a clean build; objects differ since at least 1.60; cause found (ThinLTO input in codegen completion order); [facts](hunt/thinlto-module-order.md), local stopgap: inputs sorted by name ([patch](hunt/thinlto-order-stopgap.patch)) |
| 11 | an incremental rebuild after an edit panics ("`trimmed_def_paths` called, diagnostics were expected but none were emitted") when the previous session had `-Zprint-type-sizes` and the crate has an `async fn` awaiting another | **looks new**; found by the three-way walk over untracked option transitions ([`flags.md`](flags.md)); since 1.79; cause found (an awaited type formatted with trimmed paths inside `layout_of`); [facts](hunt/print-type-sizes-trimmed-paths.md), local stopgap: the field type formatted without trimmed paths ([patch](hunt/print-type-sizes-trimmed-stopgap.patch)) |
| 12 | rustc segfaults in LLVM's DWARF emission with `-g -Crelocation-model=rwpi` on x86_64 when the crate has a writable static | **looks new**; stable flags; found by trying `-Crelocation-model` values on sink ([`flags.md`](flags.md)); since 1.60 (LLVM 14); [facts](hunt/rwpi-debuginfo-segfault.md), local stopgap: `rwpi` and `ropi-rwpi` rejected off ARM ([patch](hunt/rwpi-stopgap.patch)) |
| 13 | LLVM's machine outliner (`-Cllvm-args=-enable-machine-outliner`) segfaults with retpolines at `-Copt-level` 1 and up, and fails in other combinations (`-Zcf-protection` with `-Zpatchable-function-entry`, the large code model) | in LLVM, reproduced with `llc` alone; unstable or raw LLVM flags; found by the flag walk with `mirth-lab flag-min`; since at least 1.71; [facts](hunt/llvm-retpoline.md); excluded from the models |
| 14 | LLVM 23 lowers a retpoline tail call under the large code model to the APX instruction `jmpabs`: binaries die with SIGILL on CPUs without APX (every program that allocates), dylibs fail to link | in LLVM, a regression in LLVM 23 (fine on LLVM 22.1.8); unstable flags in rustc; reproduced with `llc` on 13 lines of IR; [facts](hunt/llvm-retpoline.md); excluded from the models |
| 15 | under the incomplete `guard_patterns` feature, a guard pattern's guard is ignored: `Some(x if x > 3)` matches `Some(2)`, and the binding cannot be used in the arm | incomplete feature; found while covering nightly syntax in sink ([`grammar.md`](grammar.md)); [facts](hunt/guard-patterns-ignored.md), no fix |
| 16 | with `-Zcache-proc-macros=yes -Zmetadata-crate-hash=no`, an incremental rebuild of a crate using derives gets a different crate hash (SVH) from a clean build after an edit upstream | unstable options (one "potentially unsound"); found by the fuzzer under walk configurations; not root-caused; [facts](hunt/cached-proc-macros-crate-hash.md); excluded from the models |
| 17 | with `-Zunleash-the-miri-inside-of-you`, the "skipping const checks" warning is not shown again on an incremental rebuild | testing-only option; found by the UI-test fuzzer ([`coverage.md`](coverage.md)); since at least 1.60; [facts](hunt/unleash-warning-lost.md); tests using the option skipped |
| 18 | after a fatal error (a missing lang item), an incremental rebuild reports fewer errors than a clean build: the fatal error is reached in a different query order | diagnostics only; found by the UI-test fuzzer; stock nightly; [facts](hunt/fatal-error-order.md); labelled known in `mirth-lab ui-fuzz` |
| 19 | on riscv64 and loongarch64, an `extern "C"` call passes an `i32` (or narrower integer) that lands on the stack without sign-extending it; a clang-compiled callee reads the slot as already extended | **looks new**; ABI, stable code; found by the ABI differential against clang ([`checks.md`](checks.md)); since at least 1.80; cause found (extension only `if *avail_gprs >= 1` in `callconv/riscv.rs`, same in `loongarch.rs`); [facts](hunt/riscv-stack-arg-extension.md) |
| 20 | on RISC-V and LoongArch hard-float targets, a `repr(C)` struct of one float and one pointer is passed in a floating-point and an integer register; clang passes it by the integer convention, so C and Rust disagree on where it is | **looks new**; ABI, stable code; found by the ABI differential; since at least 1.80; cause found (`Primitive::Pointer` counted as an integer in `should_use_fp_conv_helper`, `callconv/riscv.rs` and `loongarch.rs`); [facts](hunt/riscv-float-pointer-struct.md) |
| 21 | `-Zvalidate-mir` rejects MIR the compiler builds from accepted code: projections into `#[repr(simd)]` types (banned by MCP#838) in 9 SIMD tests, and an unsize coercion to `Pin<Box<dyn Future + Send>>` in `async-await/issue-86507.rs` | found by the internal-checks sweep (`mirth-lab crash-diff`); stock nightly with `-Zvalidate-mir`; compiletest does not validate UI tests; [facts](hunt/internal-checks.md) |
| 22 | the new trait solver trips a debug assertion in region outlives (`regions.rs:37`, `!type_outlives.has_non_rigid_aliases()`) on 5 UI tests | debug-assertion builds with nightly's default solver; hidden in CI by compiletest's solver pin; a sibling of closed #160206; [facts](hunt/internal-checks.md) |
| 23 | an `attempt to add with overflow` in `ty/instance.rs:421` compiling `recursion/issue-83150.rs` under the new solver | overflow-checked builds; hidden by the solver pin; [facts](hunt/internal-checks.md) |
| 24 | `-Zvalidate-mir` rejects a move of a dereferenced unsized place into a call (`unsized-locals/unsized-exprs2.rs`) | incomplete `unsized_fn_params`; [facts](hunt/internal-checks.md) |
| 25 | an invalid constant (E0080, `UnsafeCell` in read-only memory) is rejected when an unused `let _ = &C` is in a non-generic function, and accepted when it is in a generic one, unless `-Zmir-opt-level=0`: a MIR pass removes the promoted's last use and nothing validates it at monomorphization | **looks new**; stable code; found by the equivalent-rewrite differential (`generic-wrap`); since at least 1.80; pass located (`SimplifyLocals-before-const-prop` removes the last use), cause not narrowed further; [facts](hunt/promoted-validation-generic.md) |
| 26 | meilisearch (edition 2021) stops compiling on nightly-2026-10-06: `Ok(()) as Result<_>` now infers `!` (never-type fallback in edition 2021), and neither 1.98 nor the July nightly warned, also with the future-compatibility lints on | **looks new** as a lint false negative; found by release-to-release; reduced to 11 lines; [facts](hunt/release-regressions.md) |
| 27 | under the new trait solver (nightly's default), a type parameter that appears only in a projection (`T0::Of<'_>`) of a function-pointer coercion is not inferred (E0283); breaks surrealdb through `diskann-wide 0.54.0` | **known, intended**: `diskann-wide` is listed in #160895 ("higher-ranked associated type", the intended breakage of trait-system-refactor-initiative#168; 0.55 not yet patched); surrealdb is an affected project not on that list; found by release-to-release; [facts](hunt/release-regressions.md) |
| 28 | glob-import ambiguity depends on item order: with two modules re-exporting each other's globs, one order is E0659 and the other compiles and calls a different function (1.98, nightly); the accepted order has swapped between releases | **looks new**; stable code; found by the equivalent-rewrite differential (`reorder`) on 3 UI tests; [facts](hunt/glob-ambiguity-order.md) |
| 29 | machine-applicable lint fixes (what `cargo fix` applies unasked) break builds: `unused_variables` turns `ref b` into a moving `_b` and renames only the declaration of variables mentioned elsewhere, `unused_mut` changes one or-pattern alternative or a variable a `move` closure assigns, `unused_imports` removes a glob that resolution needs | **looks new**; stable 1.98; found by the suggestions-apply check (111 lint fixes in UI tests); six 3–7 line reductions; [facts](hunt/lint-fixes-break-builds.md) |
| 30 | compiler-internal debug output in user-facing diagnostics: under the default (new) solver, E0308 help suggests `as fn(?0t) -> ?0t`; an E0391 cycle note prints `Binder { value: ConstEvaluatable(AliasConst(… DefId(0:7 ~ …` (blessed in `offset-of/inside-array-length.stderr`) | low, diagnostics; found by the diagnostic-invariants check over 18,374 UI tests (excluding tests that ask for verbose output); the first not in CI because of the solver pin ([`solver-triage.md`](solver-triage.md) item I) |
| 31 | `#[rustc_main]` on a struct, impl, trait or module, on stable: after the expected E0658 and "cannot be used on structs", rustc ICEs ("unexpected sort of node in fn_sig()", `collect.rs`): the item is still taken as the entry point | **looks new**, low (error recovery, internal attribute); regression between 1.91.0 and 1.93.0; found by the feature-gate check; [repro](hunt/tests/rustc-main-on-struct.rs) |
| 32 | new-solver compile-time regression: a chain of N `.map()` calls type-checks in 4.5 s / 520 MB at N=200 on nightly-2026-08-03 and 37–58 s / 2.0–2.9 GB from nightly-2026-08-04, with a new "overflow evaluating the requirement `Map<…<Map<_, …>>: Iterator`" future-compat warning; nightly's default solver is the new one, so default builds regressed from 2.2 s (old solver, July) to 53 s | **looks new**, medium (compile time, realistic code shape); bisected over nightlies to #160254 (the only solver PR in the range); found by the scaling check; [facts](hunt/iter-chain-solver-regression.md) |
| 48 | gdb pretty-printers on collections of zero-sized elements: `Vec<Z>` and `&[Z]` print their size and a Python exception ("Cannot perform pointer math on incomplete type"), `VecDeque<()>` raises `ZeroDivisionError` (`% cap` with the stored capacity 0), and BTreeMap/BTreeSet show every zero-sized key or value as `()` whatever its type | **looks new**, low (debugging); since at least 1.80.0; found by the debugger round trip; [facts](hunt/gdb-printers.md), [repro](hunt/tests/debug-check/zst-printers.rs) |
| 49 | the gdb pretty-printer for `core::cell::Ref`/`RefMut` fails on every guard ("Attempt to take contents of a non-pointer value": it dereferences the `NonNull` field without unwrapping it), so neither the borrowed value nor the borrow count is shown | **looks new**, low (debugging); since at least 1.80.0; no gdb test prints a `Ref`; found by the debugger round trip; [facts](hunt/gdb-printers.md#34-ref-and-refmut), [repro](hunt/tests/debug-check/ref-printer.rs) |

Findings 1 and 2 are single-threaded: an ordinary `cargo build`, an edit, another
`cargo build`, and the metadata differs from a clean build of the edited source. Both come
from the same kind of mistake: a value that does not survive a round trip through the
incremental cache unchanged. All three reproduce with the official `nightly-2026-10-06`,
without mirth: `docs/hunt/repro.sh` runs them. None was searched for: P5 and P6 reported
them on the first run of the new fixture.

Draft bug reports for 1, 2, 4, 5, 6, 7 and 8, written to be filed upstream, are
[`hunt/issue-generics-order.md`](hunt/issue-generics-order.md),
[`hunt/issue-literal-dedup.md`](hunt/issue-literal-dedup.md) and
[`hunt/issue-stale-metadata-reuse.md`](hunt/issue-stale-metadata-reuse.md) and
[`hunt/issue-untracked-options.md`](hunt/issue-untracked-options.md), the last a comment for
rust-lang/rust#84232; for 6 it is
[`hunt/issue-stale-debuginfo-source.md`](hunt/issue-stale-debuginfo-source.md), which has no
fix, since every fix costs codegen reuse and the choice is the maintainers'. Each has a candidate fix
(`hunt/*.patch`) and a regression test in the style of rustc's `tests/run-make`
(`hunt/tests/`), which fails on the pinned compiler and passes with the fix.

With both fixes applied, P6 holds for all ten edits single-threaded, and these rustc tests
still pass: `tests/incremental`, the metadata-related UI and run-make tests,
`tests/ui/{consts,statics,const-generics}` and `tests/codegen-llvm`.

Everything else held: P1, P2, P4 and P7 on the recorded clean build; the touch-only
rebuild reused every crate's metadata; and `-Zincremental-verify-ich` found no unstable
fingerprint in 20 incremental rebuilds.

### 1. `param_def_id_to_index` order

```rust
pub struct Grid<A, B, C>(A, B, C);
impl<A, B, C> Grid<A, B, C> { pub const AREA: usize = 1; }
```

`Generics::param_def_id_to_index` is an `FxHashMap`. `HashMap`'s `Encodable` writes it in
iteration order, and `Decodable` collects it back in that order. In this example all three
keys hash to the same home bucket of a 4-bucket table, so iteration order is insertion order
rotated by one. Each time `generics_of` goes through the incremental cache, the map is
rotated again, and the metadata encoder writes it in the rotated order. The encoded order
cycles with period 3 over successive incremental rebuilds, which is exactly what a
standalone program with `std` `HashMap` and `rustc_hash` 2.1.1 predicts.

Adding a comment to the top of `lib.rs` changes the metadata of an incremental rebuild
relative to a clean one for `either`, `smallvec`, `memchr` and `arrayvec`, and making the
field an `FxIndexMap` removes the difference in all four. The same field is the cause given
in [#163878](https://github.com/rust-lang/rust/issues/163878) under `-Zthreads`.

### 2. String literals encoded twice

```rust
#[inline] pub fn a() -> &'static str { "literal" }
#[inline] pub fn b() -> &'static str { "literal" }   // then edited to `{ let s = "literal"; s }`
```

String literals get their `AllocId` from `allocate_bytes_dedup`, so both bodies share one,
and a clean build encodes the allocation once. In the rebuild, `a`'s MIR comes from the
incremental cache, and decoding a memory allocation always reserves a fresh `AllocId`
(`reserve_and_set_memory_alloc`). The metadata then encodes two allocations with identical
bytes. It started with [#116707](https://github.com/rust-lang/rust/pull/116707), which gave
`ConstValue::Slice` an `AllocId`: `nightly-2025-07-24` is not affected, `nightly-2025-07-26`
is. No effect on generated code was found.

In the fixture, the two literals were the type name in `#[derive(Debug)]`'s `fmt` (which is
`#[inline]`, so its MIR is encoded) and the same name in a `const` from the fixture's own
derive. That is why removing either derive made it disappear. The first write-up of this
finding blamed hygiene data, because the rebuild resolved foreign expansions while encoding;
that was a side effect of decoding cached MIR, not the cause.

### 3. `impl Trait` in traits under `-Zthreads`

```rust
pub trait Render { fn render(&self) -> impl Sized; }
pub trait Draw { fn draw(&self) -> impl Sized; }
```

Eight builds with `-Zthreads=8` give three or four different `.rmeta` files.
A comment on [#162202](https://github.com/rust-lang/rust/issues/162202)
reports return-position `impl Trait` in traits as not reproducible, so this
is known. The fix for finding 1 does not change it. Both fixtures have such
traits, so P5 under `-Zthreads=8` fails on `wide` every time, and on
`chain` occasionally (1 build in 24).

## Not done

- Neither new finding has been reported upstream. The drafts are ready; check #163878 again
  before filing the first, since it touches the same field.
- `fixtures/wide` fails `check.sh` (P5 under `-Zthreads=8`, finding 3) and P6 for most
  edits (findings 1 and 2) until those are fixed. Its lists are blessed against the
  unmodified compiler.

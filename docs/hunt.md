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
| 33 | rustdoc panics on an empty nested `use` group (`use {{}};`, `use {{}, {}};`) that rustc accepts: `Option::unwrap()` on `None` in `clean_use_statement_inner` (`clean/mod.rs:3224`) | **looks new**, medium (`cargo doc` crash on valid code); regression on nightly-2026-09-26, range contains #161349 ("Unflatten `use` statements in HIR"); still in nightly-2026-10-10; found by the rustdoc check; [facts](hunt/rustdoc-empty-nested-use.md) |
| 34 | rustdoc builds its session options without rustc's post-parse adjustments: `-Ccodegen-units=1` is ignored (`-Zsanitizer=cfi -Clto` rejected), and `-Zassumptions-on-binders` does not switch on the next solver (ICE: `assertion failed: self.next_trait_solver()`) | **looks new**, low (unstable flags); since at least 1.80.0 (codegen units); found by the rustdoc check; [facts](hunt/rustdoc-session-options.md) |
| 35 | rustdoc rejects an associated-const binding rustc accepts (generic const items): "anonymous constants referencing generics are not yet supported"; `clean_hir_term` types the constant with identity arguments (its own FIXME) | low (incomplete features); found by the rustdoc check; [facts](hunt/rustdoc-gca-anon-const.md) |
| 36 | rustdoc panics on an associated-const binding through a supertrait (`T: C<CONST = 2>` with `CONST` in `C`'s supertrait): the lookup searches only `C`'s own items, then `assoc_item.unwrap()` (`clean/mod.rs:539`) | **looks new**, low (incomplete features); found by the rustdoc check; [facts](hunt/rustdoc-supertrait-assoc-const.md) |
| 37 | rustdoc fails with E0080 on a type alias whose array length does not evaluate (`type D = [(); panic!()];`), which rustc accepts (aliases are not checked): `clean_ty` normalizes the length | **looks new**, low; since at least 1.80.0; found by the rustdoc check; [facts](hunt/rustdoc-alias-array-length.md) |
| 38 | rustdoc ICEs where rustc reports an error, on delayed bugs whose real error comes from a step rustdoc skips: a static too large for the target (stable, `static X: [u8; 1 << 61] = …`; rustc E0080), an invalid pattern type, a `generic_const_exprs` case | **looks new**, low (code that does not compile); stable since at least 1.80.0; found by the rustdoc check; [facts](hunt/rustdoc-delayed-bugs.md) |
| 39 | rustdoc JSON (even `--document-private-items`): an impl inside a function body names a type local to that body, and its id is in neither `index` nor `paths` (jsondoclint would reject the output) | low (JSON consumers); since at least 1.98.0; related to the open stripped-item dangling-id issues (#113674 family) but nothing is stripped here; found by the rustdoc check; [facts](hunt/rustdoc-json-body-local.md) |
| 40 | `let_underscore_drop` (allow-by-default): fires on `let _ = x;` with `x` a place, which neither moves nor drops it, and its "drop" fix moves the drop (output changes); its two machine-applicable fixes break builds: binding keeps borrowed temporaries alive (E0716), `drop(…)` loses the `let`'s type annotation (E0283) and expression attributes (`#[coroutine]`), and inside a macro rewrites the macro body (`drop()`, `drop($expr;`) | **looks new**, low (allow-by-default; `cargo fix` skips alternative suggestions, #104910); 1.98.0 and nightly; 26 UI tests; found by the lint-oracle check; [facts](hunt/lint-check.md) |
| 41 | lifetime-lint fixes that change meaning: `single_use_lifetimes` deletes a `#[may_dangle]` lifetime but not its attribute, which moves the unsafe promise to the next parameter and still compiles; it turns a derive field's `for<'a> fn(T::A<'a>)` into `'_` (E0637); `unused_lifetimes` removes the `for<'a>` that kept `where for<'a> Inherent: Clone` from being checked (E0277) | **looks new**, low (allow-by-default lints; `may_dangle` is unstable); 1.98.0 and nightly; found by the lint-oracle check; [facts](hunt/lint-check.md) |
| 42 | `dead_code` reports needed items as never used: a trait used only in the where-clause or a projection in the self type of an impl whose methods are called (stable since at least 1.80.0); the `#[define_opaque]` function that is an opaque type's only defining use (removing it: "unconstrained opaque type") | **looks new**; trait cases on stable (warn-by-default), opaque case nightly-only; 3 + 24 UI tests (one blesses the warning); found by the lint-oracle check; [facts](hunt/lint-check.md) |
| 43 | `trivial_numeric_casts` calls `5 as i16` an `i16`-to-`i16` cast, but the cast is what makes the literal `i16`: without it the program uses `i32` (prints 4, not 2; a `transmute` size mismatch in a UI test) | **looks new**, low (allow-by-default); 1.98.0 and nightly; found by the lint-oracle check; [repro](hunt/tests/lint-check/trivial-numeric-cast-literal.rs) |
| 44 | mips64 (n64): narrow integer `extern "C"` arguments lose `signext`/`zeroext`, in registers too; a Rust caller passes `x as i32` / `x as i8` without `sll`/`seb`, where the convention and clang/GCC callees expect sign extension | **looks new**, high for the targets (silent wrong values in C callees), tier 3; regression from #163653 (merged 2026-10-04): present in nightly-2026-10-06, absent in nightly-2026-07-18 and 1.98.0; found by `abi-diff --asm`; [facts](hunt/mips64-narrow-int-extension.md) |
| 45 | thumbv7a-{pc,uwp}-windows-msvc: homogeneous float aggregates (e.g. `struct { float a, b; }`) are passed in core registers and returned through memory, where clang uses s0/s1 (AAPCS VFP rules); rustc applies the VFP aggregate rules only to `eabihf` targets | **looks new**, tier 3; since at least 1.80.0; found by `abi-diff --asm`; [facts](hunt/windows-arm32-vfp-aggregates.md) |
| 46 | powerpc-unknown-{freebsd,netbsd,openbsd,helenos}: aggregates of up to 8 bytes are returned through memory, where clang returns them in r3/r4 (on Linux both use memory) | **looks new**, tier 3; since at least 1.80.0; FreeBSD's powerpc system compiler is clang; found by `abi-diff --asm`; [facts](hunt/powerpc-bsd-struct-return.md) |
| 47 | m68k: `repr(C)` alignment of `i64` is 4 and of `f64` 8, where GCC's documented m68k default is 2 (clang uses 8 for both), so structs containing them are laid out differently | tier 3; related to open #117252 (pointer alignment); found by `abi-diff --asm`; [facts](hunt/m68k-repr-c-alignment.md) |
| 48 | gdb pretty-printers on collections of zero-sized elements: `Vec<Z>` and `&[Z]` print their size and a Python exception ("Cannot perform pointer math on incomplete type"), `VecDeque<()>` raises `ZeroDivisionError` (`% cap` with the stored capacity 0), and BTreeMap/BTreeSet show every zero-sized key or value as `()` whatever its type | **looks new**, low (debugging); since at least 1.80.0; found by the debugger round trip; [facts](hunt/gdb-printers.md), [repro](hunt/tests/debug-check/zst-printers.rs) |
| 49 | the gdb pretty-printer for `core::cell::Ref`/`RefMut` fails on every guard ("Attempt to take contents of a non-pointer value": it dereferences the `NonNull` field without unwrapping it), so neither the borrowed value nor the borrow count is shown | **looks new**, low (debugging); since at least 1.80.0; no gdb test prints a `Ref`; found by the debugger round trip; [facts](hunt/gdb-printers.md#34-ref-and-refmut), [repro](hunt/tests/debug-check/ref-printer.rs) |
| 50 | `-Zassumptions-on-binders` crashes on any crate that enables `generic_const_exprs`: `#![feature(generic_const_exprs)] fn main() {}` panics with "assertion failed: self.next_trait_solver()" (`rustc_infer/src/infer/outlives/obligations.rs:280`, in `check_well_formed`; also from `param_env`, `type_of`, `adt_destructor`, `mir_borrowck`); a trait with a `T: Freeze + 'static` parameter gives the sibling assertion `!self.tcx.assumptions_on_binders()` (line 165); with `-Znext-solver`, two overlapping `min_specialization` impls give a delayed bug "`[…]` is not fully resolved" | **looks new** (none of the 8 open assumptions-on-binders ICE issues; tracking #158130), low (experimental flag); nightly-2026-07-18 (line 241) and 10-06; found by `gate-mutate` (165 of its 197 new-signature mutants); [repro](hunt/tests/gate-mutate/assumptions-gce.rs), [repro](hunt/tests/gate-mutate/assumptions-specialization.rs) |
| 51 | `generic_const_exprs` with gca generic const items under `-Znext-solver`: `const ARR: [(); gca!(ADD1::<0>)] = [(); gca!(INC::<0>)]` with `ADD1`/`INC` generic consts gives the delayed bug "encountered regular consts in the old solver's const normalization" | **looks new** (no issue with the message; same shape as open #153735, a different ICE), low (two incomplete features); nightly-2026-10-06 (the gca_* gates are newer than nightly-2026-07-18); found by `gate-mutate` (an extra `generic_const_exprs` gate on `const-generics/gca/basic-different-definitions.rs`); [repro](hunt/tests/gate-mutate/gce-gca-old-solver-normalization.rs) |
| 52 | "expected branch, got Leaf(0x01)" (`rustc_type_ir/src/const_kind.rs:256`, `valtree_to_const_val`) for a tuple constant with a mistyped field, with `gca_adts`, `gca_macroless_items` and `gca_min_const_items` together; no type error is reported first. #162392's program, fixed and closed 2026-09-14; its regression test (`mgca/valtree-leaf-const.rs`) uses `gca_macroless_args` and passes; with any two of the three gates the program gets the expected E0308 | **looks new** (a route the fix of closed #162392 does not cover), low; nightly-2026-10-06; found by `gate-mutate` (a splice); [repro](hunt/tests/gate-mutate/gca-adts-leaf.rs) |
| 53 | "AliasConst::type_of got InherentSelf - args should always be InherentImpl at this point" (`const_kind.rs:85`, `check_well_formed`) for `fn to_bytes() -> [u8; gca!(Self::SIZE)]` in an inherent impl, when `generic_const_exprs` is also enabled; without it the program is accepted as in #162147's regression test (`gca/wf-inherentimpl.rs`) | **looks new** (a route around closed #162147, fixed 2026-09-03), low; nightly-2026-10-06; found by `gate-mutate` (a splice whose `--cfg full` turns on `generic_const_exprs`); [repro](hunt/tests/gate-mutate/gce-inherent-self.rs) |
| 54 | `rustc --test` panics "expected statement" (`rustc_expand/src/base.rs:172`) after E0736 for a `#[test] #[unsafe(naked)] extern "C" fn` nested inside another function's body; a `#[test]` inner fn without `naked` only warns "cannot test inner items" | **looks new** (closed issues with the message: #112360, #109816 (both `--test`), #83469, #149980; none open, none with `naked`), low (error recovery), **stable**: 1.82.0 through 1.88.0, 1.90.0, 1.98.0 and nightly-2026-10-06 (each tested) (1.81.0 rejects with E0658/E0787; before 1.88 the panic comes before the gate error); found by `gate-mutate` (an item moved into a generic fn); [repro](hunt/tests/gate-mutate/naked-test-inner-fn.rs) |
| 55 | a hang under the new trait solver: a closure with a `for<'a, 'b>` binder returning a TAIT with two lifetimes, passed where a `for<'a> AsyncFn<&'a mut C, …>` bound (a trait with an `FnMut` supertrait and an associated future) is required, does not finish compiling (still running after 200 s); with `-Znext-solver=coherence` (the old solver) it reports E0046/E0308/E0277 in 0.05 s | **looks new** (no issue found), medium: the new solver is nightly's default, so the plain `rustc` hangs on nightly-2026-10-06; also hangs on nightly-2026-07-18 with `-Znext-solver=globally` (not a recent regression); found by `gate-mutate` (a module splice of two tests); [repro](hunt/tests/gate-mutate/next-solver-hang.rs) |
| 58 | `-Zunleash-the-miri-inside-of-you`: an incremental rebuild drops the "skipping const checks" warning and the error that the flag may not circumvent feature gates (session state filled as a side effect of const checking), so a rebuild accepts a crate a clean build rejects | **looks new**, low (a flag for testing the const evaluator); the warning since at least 1.80.0, the error on nightly-2026-10-06; found by `ui-incr`; [facts](hunt/unleash-incremental.md), [repro](hunt/tests/unleash-gate-incremental.rs) |

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

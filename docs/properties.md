# Properties from 1,000 rustc bugs

The 1,000 most recently closed rust-lang/rust issues labelled `C-bug` that a merged
PR closed (2025-12-10 to 2026-10-05; 480 of them ICEs) were read by ten agents,
100 each, looking for invariants that a past bug violated and that a mechanical check
could test on any crate, without knowing the bug. 94 candidates were merged into 30
properties. A second pass checked every cited issue against its text and kept only the
issues that really violate the property: 107 of 156 citations survived. The crash
baseline (no ICE, hang or stack overflow; 125 issues) was not checked this way, since any
harness detects a crash.

The agents were small models and the checking was done by small models too. Treat the
issue lists as leads, not proof: the checking rejected most of the original citations
for some properties, and it also accepted at least one citation that does not fit
(#159677 under "incremental equals clean": it is about the library search path).

| # | property | kind | issues | cost | mirth |
|---|---|---|---|---|---|
| 1 | [Interned type-system values are well-formed](#1) | invariant | 12 | cheap | Good |
| 2 | [Old and next trait solvers agree](#2) | differential | 11 | moderate | Good |
| 3 | [extern "C" ABI matches the platform C compiler](#3) | differential | 9 | moderate | Low |
| 4 | [MIR passes validation after every pass at every mir-opt-level](#4) | invariant | 8 | cheap | Moderate |
| 5 | [Library operations stay sound under injected panics and allocation failures](#5) | semantic | 7 | expensive | Poor |
| 6 | [rustdoc accepts every crate rustc accepts](#6) | differential | 6 | cheap | Low to moderate |
| 7 | [Every span is valid](#7) | invariant | 6 | cheap | Good for the metadata half (hook span encoding) |
| 8 | [Semantically neutral edits do not change results](#8) | differential | 5 | moderate | Moderate |
| 9 | [Layout views agree](#9) | invariant | 4 | cheap | Good |
| 10 | [rustdoc output is the same however an item is re-exported](#10) | differential | 4 | cheap | Poor |
| 11 | [P6+ incremental equals clean (outputs and diagnostics)](#11) | differential | 3 | moderate | Excellent |
| 12 | [Parallel front end is deterministic](#12) | differential | 3 | cheap | Good |
| 13 | [Results agree across opt-level, mir-opt-level, codegen-units and LTO](#13) | semantic | 3 | expensive | Low |
| 14 | [A successful compile contains no error types](#14) | invariant | 3 | cheap | Good |
| 15 | [Each encoded record is written once and hashed after it is final](#15) | invariant | 3 | cheap | Excellent |
| 16 | [#[expect] is fulfilled exactly when the lint fires](#16) | differential | 3 | moderate | Low |
| 17 | [Metadata reads hit entries the writer wrote](#17) | invariant | 2 | moderate | Excellent |
| 18 | [Query results contain no inference variables](#18) | invariant | 2 | cheap | Excellent |
| 19 | [Machine-applicable suggestions apply cleanly](#19) | differential | 2 | moderate | Poor |
| 20 | [Polonius accepts at least what NLL accepts, and no more than is sound](#20) | differential | 2 | moderate | Low |
| 21 | [Unstable syntax is gated before expansion](#21) | invariant | 2 | cheap | Poor |
| 22 | [P5+ metadata determinism under irrelevant perturbation](#22) | differential | 1 | cheap | Excellent |
| 23 | [P4+ no query reads untracked state](#23) | invariant | 1 | cheap | Excellent |
| 24 | [Symbol names are injective and stable](#24) | invariant | 1 | cheap | Good |
| 25 | [dyn types have a dyn-compatible principal](#25) | invariant | 1 | cheap | Good |
| 26 | [Built-in attributes reject malformed arguments](#26) | invariant | 1 | cheap | Poor |
| 27 | [Advertised target features exist in LLVM](#27) | invariant | 1 | cheap | Poor |
| 28 | [Eq and Hash agree for std types](#28) | semantic | 1 | cheap | None |
| 29 | [Each diagnostic is emitted once](#29) | invariant | 0 | cheap | Poor |

<a id="1"></a>
## 1. Interned type-system values are well-formed

**Statement.** Every interned GenericArgs-carrying value (TraitRef, AliasTy, FnDef, etc.) must have arguments whose count and kind match the corresponding generics_of definition. Every interned const value must have a valtree whose structure matches its type. No ParamEnv must contain two predicates with the same parameter (e.g., ConstArgHasType) that specify different constraints.

**Check.** In an instrumented release-mode rustc, run the existing debug-only checks at every intern site: debug_assert_args_compatible, a valtree-vs-type conformance check, and a ParamEnv duplicate/conflict scan when param_env is built. Run on the corpus plus the UI suite.

**Cost:** cheap per crate (checks at intern sites). **Kind:** invariant. **Subsystem:** type-system / const generics.

**Tested today?** Some of these checks exist as debug_assert in debug compilers only. Valtree/type conformance and ParamEnv conflicts are not checked at all. The issues were found as ICEs later in the pipeline.

**mirth.** Good. Inject the checks at the mk_* or intern call sites and record the value, the caller's query and the DefId on failure. This turns a late ICE into a report at the point of creation.

**Bugs that violated it:**

- [#150506](https://github.com/rust-lang/rust/issues/150506) ICE: valtree: `expected leaf, got Value`
- [#150712](https://github.com/rust-lang/rust/issues/150712) ICE: `expected branch, got Leaf`
- [#150734](https://github.com/rust-lang/rust/issues/150734) ICE `expected leaf, got Value`
- [#151126](https://github.com/rust-lang/rust/issues/151126) ICE with --emit=mir :` expected ConstKind::Value, got X/#0`
- [#158675](https://github.com/rust-lang/rust/issues/158675) [ICE]: did not expect duplicate `ConstParamHasTy` for `N/#1` in param-env: ParamEnv {
- [#157189](https://github.com/rust-lang/rust/issues/157189) [ICE]: `args not compatible with generics for Borrow`
- [#137084](https://github.com/rust-lang/rust/issues/137084) mgca: index out of bounds
- [#150673](https://github.com/rust-lang/rust/issues/150673) ICE: delegation: index out of bounds
- [#150714](https://github.com/rust-lang/rust/issues/150714) ICE: abi: index out of bounds (`offsets[FieldIdx::new(i)]`)
- [#150841](https://github.com/rust-lang/rust/issues/150841) ICE `const tuple must have a tuple type`
- [#151024](https://github.com/rust-lang/rust/issues/151024) ICE `const array must have an array type`
- [#151186](https://github.com/rust-lang/rust/issues/151186) ICE：index out of bounds: the len is 0 but the index is 0

<a id="2"></a>
## 2. Old and next trait solvers agree

**Statement.** The next-solver and old solver must behave identically on any crate: accepting and rejecting the same items, inferring the same types for each body (up to region erasure), and selecting the same impls. The cited issues are cases where this invariant was violated, with the next-solver either panicking (ICE) or disagreeing with the old solver on acceptance/rejection of code.

**Check.** Build the corpus with both solvers. Compare exit status and diagnostics, and per body DefPath compare typeck_results (node types, method resolutions) and codegen Instances. Every disagreement is a bug in one of the two solvers.

**Cost:** moderate (two full typechecks). **Kind:** differential. **Subsystem:** trait-system.

**Tested today?** The next-solver CI job runs the UI suite with the next solver, and crater runs happen occasionally. Per-body inferred types are never compared.

**mirth.** Good. mirth can record typeck_results and selected impls per DefPath in both configurations, which gives a precise diff instead of only accept/reject.

**Bugs that violated it:**

- [#102580](https://github.com/rust-lang/rust/issues/102580) Overflow when deriving Clone on a struct with a recursive GAT
- [#90950](https://github.com/rust-lang/rust/issues/90950) HRTB bounds not resolving correctly (take 3, lifetimes on the RHS)
- [#152789](https://github.com/rust-lang/rust/issues/152789)  `-Znext-solver`: trait object candidate ICE "could not replace AliasTerm"
- [#151329](https://github.com/rust-lang/rust/issues/151329) [ICE]: `could not replace AliasTerm` (unsatisifed bounds)
- [#151957](https://github.com/rust-lang/rust/issues/151957) ICE: `entered unreachable code: PointeeSized is removed during lowering` with `-Z next-solver=globally` and recursive associated type bound
- [#151323](https://github.com/rust-lang/rust/issues/151323) [ICE]: !tcx.next_trait_solver_globally()
- [#151322](https://github.com/rust-lang/rust/issues/151322) [ICE]: !self.tcx.next_trait_solver_globally()
- [#151318](https://github.com/rust-lang/rust/issues/151318) [ICE]:  error performing operation: query type op
- [#138274](https://github.com/rust-lang/rust/issues/138274) [bug] When I Use tauri-plugin-http and reqwest either, I got a panic
- [#137916](https://github.com/rust-lang/rust/issues/137916) ICE Unsize coercion, but `Box<{async block@file.rs}>` isn't coercible to `Box<dyn Send>`
- [#151462](https://github.com/rust-lang/rust/issues/151462) [ICE]: `Inconsistent rustc_transmute::is_transmutable(...) result, got Yes`

<a id="3"></a>
## 3. extern "C" ABI matches the platform C compiler

**Statement.** For every target, a function signature with C-compatible types (unions, structs with floats, bool, homogeneous vector aggregates, and other aggregates) used in extern \"C\" functions must be lowered to argument passing and return value handling (register vs stack, indirect vs direct, sign/zero extension) that matches the behavior of clang/gcc for equivalent C declarations.

**Check.** Generate random C-compatible type signatures, emit matching Rust and C, and cross-call them in both directions (abi-cafe style). Run natively or under qemu for each tier-1/2 target. Optionally diff rustc's FnAbi against clang's LLVM IR attributes per parameter.

**Cost:** moderate (cross targets need qemu). **Kind:** differential. **Subsystem:** codegen / abi.

**Tested today?** tests/ui/abi/compatibility.rs and per-target codegen tests check fixed cases. abi-cafe is not in CI. Most of these bugs were reported by users on less common targets (SPARC, PowerPC, LoongArch).

**mirth.** Low. This is an output-level differential with C compilers. mirth could dump the FnAbi of every extern fn to drive the comparison, but the core harness is abi-cafe plus qemu.

**Bugs that violated it:**

- [#121408](https://github.com/rust-lang/rust/issues/121408) Clang vs wasm32-{emscripten,wasi} rustc C ABI mismatch w.r.t. "singleton" unions
- [#162011](https://github.com/rust-lang/rust/issues/162011) ABI mismatch on powerpc64 (elfv1) for union containing floats
- [#163074](https://github.com/rust-lang/rust/issues/163074) `check_abi.rs` fails due to ArgAttributes mismatch with the ABI on LoongArch64
- [#122620](https://github.com/rust-lang/rust/issues/122620) sparc64 has incorrect ABI for struct containing f64 and f32
- [#115399](https://github.com/rust-lang/rust/issues/115399) ICE in sparc64 `fn_abi_of_instance`
- [#147883](https://github.com/rust-lang/rust/issues/147883) "Size::sub: 0 - 8 would result in negative size" ICE on sparc
- [#159244](https://github.com/rust-lang/rust/issues/159244) Miscompilation with FFI `bool` return type on AArch64
- [#43894](https://github.com/rust-lang/rust/issues/43894) struct pass-by-value failing on SPARC
- [#161382](https://github.com/rust-lang/rust/issues/161382) Mishandling of AAPCS64 Homogeneous Vector Aggregates (HVAs) on aarch64-unknown-linux-gnu

<a id="4"></a>
## 4. MIR passes validation after every pass at every mir-opt-level

**Statement.** MIR optimization passes must preserve well-formedness invariants: const arguments must have the types their parameters declare, locals must only be accessed between their StorageLive and StorageDead markers, and place types as well as const values must be fully normalized. This holds at all -Zmir-opt-level settings (0, 2, 4) and with -Zinline-mir, and can be verified mechanically by running -Zvalidate-mir -Zlint-mir on real crates.

**Check.** Build every crate in the corpus with -Zvalidate-mir -Zlint-mir -Zmir-opt-level={0,2,4} -Zinline-mir and -Cdebug-assertions on the compiler. Any validation failure counts. This uses existing flags; you only need to run them on real crates.

**Cost:** cheap (existing flags; one extra build per opt-level). **Kind:** invariant. **Subsystem:** mir-build / mir-opt / const-eval.

**Tested today?** mir-opt tests use the validator, but UI tests and real crates do not run with -Zvalidate-mir by default, and mir-opt-level=4 is rarely exercised on real code.

**mirth.** Moderate. No MIR rewriting is needed because the flags exist. mirth helps by recording which pass first broke validation, and by giving every report one shared corpus and harness.

**Bugs that violated it:**

- [#156409](https://github.com/rust-lang/rust/issues/156409) [ICE]: `CTFE tried to evaluate type-const`
- [#154750](https://github.com/rust-lang/rust/issues/154750) [ICE]: `attempting to project to field at offset 0 with size 8 into immediate with layout TyAndLayout`
- [#154748](https://github.com/rust-lang/rust/issues/154748) [ICE]: invalid field access on immediate
- [#152962](https://github.com/rust-lang/rust/issues/152962) [ICE]: mgca: broken mir `Failed subtyping u8 and usize`
- [#158231](https://github.com/rust-lang/rust/issues/158231) SimplifyComparisonIntegral introduces access to a dead local variable
- [#151647](https://github.com/rust-lang/rust/issues/151647) ICE: mGCA+GCI: Broken MIR: equate_normalized_input_or_output: NoSolution
- [#151579](https://github.com/rust-lang/rust/issues/151579) ICE with `-Znext-solver` when accessing hir place
- [#120811](https://github.com/rust-lang/rust/issues/120811) ICE: Broken MIR: NoSolution

<a id="5"></a>
## 5. Library operations stay sound under injected panics and allocation failures

**Statement.** Library operations (particularly BTreeMap, Arc, Box, and array functions) fail to maintain soundness invariants when user callbacks (comparators, closures) panic or when allocations fail. Violations include: improper drops of unprocessed values, state corruption leading to double-frees, use-after-free from incorrect strong count management, and aliasing violations when custom allocators or mutable reference derivation are involved. Additionally, allocators that return pointers with provenance exceeding the requested size can be unsoundly treated as having restricted provenance, violating the deallocation contract.

**Check.** Run library tests and property tests under Miri with fault injection: closures and comparators that panic on the Nth call (for every N), a failing allocator, and a counting Drop type. Check for no Miri UB, drops equal to constructions, and a structure that is still usable or droppable afterwards.

**Cost:** expensive (Miri, N-sweep). **Kind:** semantic. **Subsystem:** library (alloc/core/std).

**Tested today?** Library tests run under Miri in CI, but there is no systematic sweep that panics at every callback position or fails allocation at every point.

**mirth.** Poor. This tests the library, not the compiler. Use Miri plus a fault-injection harness. mirth is not needed.

**Bugs that violated it:**

- [#162720](https://github.com/rust-lang/rust/issues/162720) `Arc::new_cyclic_in` uses pointer derived from mutable reference unsoundly
- [#162719](https://github.com/rust-lang/rust/issues/162719) `Box::into_unique(self) -> (Unique<T>, A)` goes through `&mut *ptr`
- [#158165](https://github.com/rust-lang/rust/issues/158165) BTreeMap::split_off is not panic-safe leading to a potential double-free
- [#157203](https://github.com/rust-lang/rust/issues/157203) Unsoundness in `Arc::make_mut` if `handle_alloc_error` unwinds
- [#155746](https://github.com/rust-lang/rust/issues/155746) UAF in allocator-backed `Arc::make_mut` after caught unwind
- [#152211](https://github.com/rust-lang/rust/issues/152211) `array::map` and `array::try_map` do not drop ZSTs properly
- [#160815](https://github.com/rust-lang/rust/issues/160815) `Condvar` and `Mutex` in `std::sys::pal::unix::sync` violate aliasing rules

<a id="6"></a>
## 6. rustdoc accepts every crate rustc accepts

**Statement.** rustdoc must not ICE on any Rust code that passes `cargo check`, particularly code involving type-relative paths and anon consts with `--generate-link-to-definition`, and must not allow proc-macro-generated spans to overwrite source spans in documentation links

**Check.** For each crate in the corpus, run cargo check, then cargo rustdoc with each mode. Any rustdoc failure on a crate that checks is a bug. Validate the JSON output against rustdoc-types.

**Cost:** cheap. **Kind:** differential. **Subsystem:** rustdoc.

**Tested today?** docs.rs and crater cover the default HTML mode. --generate-link-to-definition and JSON on real crates are barely exercised.

**mirth.** Low to moderate. This is mostly a harness differential. mirth could record which TypeckResults body was consulted for each path in order to check that it is the owning body.

**Bugs that violated it:**

- [#156418](https://github.com/rust-lang/rust/issues/156418) rustdoc ICEs on free calls, method calls & type-relative paths in anon consts under `--generate-link-to-definition`
- [#149089](https://github.com/rust-lang/rust/issues/149089) ICE when documenting embedded-io with nightly: `node HirId(...) cannot be placed in TypeckResults`
- [#150153](https://github.com/rust-lang/rust/issues/150153) ICE: rustdoc: `node HirId cannot be placed in TypeckResults with hir_owner DefId`
- [#147882](https://github.com/rust-lang/rust/issues/147882) Internal compiler error when building docs for `serde_with@3.14.1` on nightly
- [#147057](https://github.com/rust-lang/rust/issues/147057) rustdoc: ICE: [trying to look up a HirId in the wrong context]
- [#158050](https://github.com/rust-lang/rust/issues/158050) [rustdoc] link to definition doesn't generate link to item's doc when clicking on its name

<a id="7"></a>
## 7. Every span is valid

**Statement.** Every span in diagnostics (including suggestion parts) and in encoded metadata has lo <= hi (non-empty), lies inside an existing SourceFile with matching context, falls on UTF-8 character boundaries, and each suggestion's substitutions do not overlap.

**Check.** Two checks. (1) Post-process --error-format=json for every corpus crate, plus a corpus with multibyte identifiers and strings: check byte ranges against the file contents and check that suggestion parts are disjoint. (2) With mirth, validate every span encoded into rmeta against the SourceMap when it is written.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** span / diagnostics / metadata.

**Tested today?** There is an internal assertion when a bad span is sliced. Nothing validates spans systematically, and multibyte source is rare in tests.

**mirth.** Good for the metadata half (hook span encoding). The diagnostic half needs only a JSON post-processor.

**Bugs that violated it:**

- [#151610](https://github.com/rust-lang/rust/issues/151610) [ICE]: `Span must not be empty and have no suggestion`
- [#151607](https://github.com/rust-lang/rust/issues/151607) [ICE]: ` all spans must be disjoint`
- [#147339](https://github.com/rust-lang/rust/issues/147339) ICE: `span context mismatch`
- [#131292](https://github.com/rust-lang/rust/issues/131292) ICE: `bpos.to_u32() >= mbc.pos.to_u32() + mbc.bytes as u32`
- [#156316](https://github.com/rust-lang/rust/issues/156316) [ICE]: `bpos.to_u32() >= mbc.pos.to_u32() + mbc.bytes as u32`
- [#155037](https://github.com/rust-lang/rust/issues/155037) [ICE]: `bpos.to_u32() >= mbc.pos.to_u32() + mbc.bytes as u32`

<a id="8"></a>
## 8. Semantically neutral edits do not change results

**Statement.** The Rust compiler may incorrectly identify certain edits as semantically neutral (particularly parentheses in patterns, braces affecting resource lifetime scope, and glob re-exports) when they are actually required for compilation or correct behavior, and may fail to recognize trait implementations on type aliases and projections that normalize to an underlying type as equivalent to direct implementations on that type.

**Check.** Apply automatic rewrites from a fixed catalogue to corpus crates. Compare exit status, the multiset of diagnostics keyed by (code, lint, item DefPath), and rmeta after span normalization.

**Cost:** moderate. **Kind:** differential. **Subsystem:** resolve / lints / trait-system / macros.

**Tested today?** UI tests check isolated forms. No transformation-based differential testing exists.

**mirth.** Moderate. mirth can compare per-query results keyed by DefPath between the original and the rewritten build, which is finer-grained than diagnostics.

**Bugs that violated it:**

- [#86959](https://github.com/rust-lang/rust/issues/86959) Unnecessary parentheses warning for (A | B) as :pat in 2018 edition
- [#160741](https://github.com/rust-lang/rust/issues/160741) "unnecessary braces around `for` iterator expression" have effect on program behavior
- [#157758](https://github.com/rust-lang/rust/issues/157758) False positive of lint `missing_debug_implementations` if `Debug` is implemented for an alias type (e.g., projection) that can get normalized to the relevant type
- [#157757](https://github.com/rust-lang/rust/issues/157757) False positive of lint `missing_debug_implementations` if `Debug` is implemented for a free alias type that expands to the relevant type
- [#152004](https://github.com/rust-lang/rust/issues/152004) Regression on nightly: unused pub(crate) use::*; is not actually unused

<a id="9"></a>
## 9. Layout views agree

**Statement.** Unsafe binder layout checks and discriminant helpers must use the inner type's layout view. Transmute's SizeSkeleton check must account for repr(align/packed) differences that affect a type's actual size. repr(transparent) wrappers have the same ABI as their non-ZST field.

**Check.** With mirth, record layout_of results per type. At each transmute check and each transparent or wrapper type, compare against layout_of of the inner type and flag disagreements. Also feed a generated corpus of repr(align/packed/C/transparent) types.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** layout / type-system.

**Tested today?** There are debug assertions in layout code and transmute UI tests. Nothing cross-checks SizeSkeleton against layout_of.

**mirth.** Good. Hook layout_of and SizeSkeleton::compute and compare their results.

**Bugs that violated it:**

- [#154426](https://github.com/rust-lang/rust/issues/154426) [ICE]: `Scalar` layout for non-primitive non-enum type unsafe
- [#154424](https://github.com/rust-lang/rust/issues/154424) [ICE]: discriminant_for_variant() is None
- [#155412](https://github.com/rust-lang/rust/issues/155412) transmute size check is wrong for overaligned newtype
- [#88290](https://github.com/rust-lang/rust/issues/88290) Transmute special-case doesn't take into consideration alignment or enum repr.

<a id="10"></a>
## 10. rustdoc output is the same however an item is re-exported

**Statement.** Re-exported items may fail to preserve the documentation and cfg badges of their original definitions, particularly: multi-level re-exports may lose documentation from intermediate levels, glob re-exports may fail to include items that appear in named re-exports, and cfg badges may not propagate correctly to glob re-exports or re-exported type aliases.

**Check.** Use rustdoc JSON. For every re-exported item in the corpus, compare the inlined item's docs, cfg and signature with the original item's (outer docs and cfg added at the re-export are allowed).

**Cost:** cheap. **Kind:** differential. **Subsystem:** rustdoc.

**Tested today?** rustdoc tests cover specific re-export shapes only.

**mirth.** Poor. It is a JSON post-processor.

**Bugs that violated it:**

- [#81893](https://github.com/rust-lang/rust/issues/81893) Documentation of a re-export doesn't appear on level-two re-export
- [#53724](https://github.com/rust-lang/rust/issues/53724) `pub use serde::*` doesn't show traits in `cargo doc`
- [#96166](https://github.com/rust-lang/rust/issues/96166) doc(cfg) doesn't work on glob reexports
- [#154921](https://github.com/rust-lang/rust/issues/154921) `doc(auto_cfg)` and `doc(cfg)` don't add cfgs to re-exported type aliases

<a id="11"></a>
## 11. P6+ incremental equals clean (outputs and diagnostics)

**Statement.** Incremental builds can produce different .rmeta bytes (due to iteration-order-dependent content like DocLinkResMap when encountering unrelated files in the library search path) and duplicate diagnostics compared to clean builds (due to span-based deduplication issues).

**Check.** Use a script per crate: clean build; then incremental builds after (a) a no-op touch, (b) a body edit, (c) a signature edit, (d) reverting to the original. Byte-compare each result with a clean build of the same source. Also compare the --error-format=json diagnostic multisets. A diagnostic that is missing or duplicated only in the incremental build is a failure.

**Cost:** moderate (several builds per crate). **Kind:** differential. **Subsystem:** incremental.

**Tested today?** tests/incremental checks that specific revisions compile or fail and that specific nodes are clean or dirty. It does not compare bytes against a clean build, and it does not compare diagnostics. mirth's P6 does byte comparison for single edits only.

**mirth.** Excellent. P6 exists. Extend it with revert/no-op sequences and a diagnostic diff. mirth's record of which queries were re-executed versus loaded from cache attributes a divergence to a cached query.

**Bugs that violated it:**

- [#162901](https://github.com/rust-lang/rust/issues/162901) Diagnostic deduplication breaks with incr comp
- [#159677](https://github.com/rust-lang/rust/issues/159677) `.rmeta` contents depend on unrelated files in the library search path
- [#106571](https://github.com/rust-lang/rust/issues/106571) Regression: duplicate messages appear in --error-format=json

<a id="12"></a>
## 12. Parallel front end is deterministic

**Statement.** The parallel front end with -Zthreads > 1 produces non-deterministic behavior in: (1) encoding of syntax contexts affecting derived code generation, (2) LLVM inline asm location cookies in bitcode/LTO output, and (3) query cycle handling in the deadlock resolver. These cause byte-identical outputs and repeated runs to diverge from both -Zthreads=1 and each other.

**Check.** Build the corpus with -Zthreads=1 once and -Zthreads=8 three times. Byte-compare outputs and diff the JSON diagnostics. When they differ, compare mirth's per-table write logs.

**Cost:** cheap. **Kind:** differential. **Subsystem:** parallel front end.

**Tested today?** A parallel-rustc CI job runs some UI tests. Outputs are not compared against single-threaded builds.

**mirth.** Good. It reuses the P5 harness with a different flag, and mirth's write logs localize the divergence.

**Bugs that violated it:**

- [#129094](https://github.com/rust-lang/rust/issues/129094) derives: parallel compiler makes builds irreproducible 
- [#150451](https://github.com/rust-lang/rust/issues/150451) parallel compiler: `threads::spawn`ning loop not reproducible
- [#153391](https://github.com/rust-lang/rust/issues/153391) [ICE]: parallel: None in compiler/rustc_type_ir/src/ty_kind.rs

<a id="13"></a>
## 13. Results agree across opt-level, mir-opt-level, codegen-units and LTO

**Statement.** Compiler optimization passes and LTO can silently change program semantics: a program's observable behavior (exit status, panic status, and linkability) may differ across -Copt-level 0/3, -Zmir-opt-level 0/4, codegen-units 1/16, and lto off/thin/fat settings, when it should remain identical.

**Check.** Run cargo test for crates in the corpus under a configuration matrix and diff per-test outcomes. Flag link failures that appear in only one configuration. Run Miri on a subset as the reference.

**Cost:** expensive (matrix of full builds plus test runs). **Kind:** semantic. **Subsystem:** mir-opt / codegen / LTO.

**Tested today?** Individual mir-opt tests exist. There is no systematic matrix over real crates' test suites, and LTO/codegen-unit link failures are found by users.

**mirth.** Low. It is an output-level differential. mirth could record which MIR pass changed a function that later misbehaves, which helps bisect the cause.

**Bugs that violated it:**

- [#163779](https://github.com/rust-lang/rust/issues/163779) SsaRangePropagation propagates range information from optional asserts
- [#162348](https://github.com/rust-lang/rust/issues/162348) 1.99 beta crater regression: SIGSEGV in LLVM in release mode
- [#153645](https://github.com/rust-lang/rust/issues/153645) Externally Implementable Items: `error: undefined symbol` when opt-level >= 1

<a id="14"></a>
## 14. A successful compile contains no error types

**Statement.** If a compilation exits 0, no ty::Error, ConstKind::Error, or ErrorGuaranteed-carrying value should appear in compiler internals (valtree constants, transmute layout checking, MIR, query results, or metadata). Error types are created only after errors are emitted; violations occur when compiler subsystems construct these types during failed operations (const evaluation, layout normalization, or delegation) without emitting corresponding errors or errors without reporting.

**Check.** With mirth, hook the construction of Ty::new_error, Const::new_error and region errors and record the creation site. When the session ends with zero errors, any recorded creation is a bug. Also scan encoded metadata for error types.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** type-system / diagnostics.

**Tested today?** ErrorGuaranteed makes forging an error hard. Delayed bugs ICE at session end only if they were delayed. Silent paths that drop an error are not checked.

**mirth.** Good. It needs one hook at the error constructors and a check at session end.

**Bugs that violated it:**

- [#150969](https://github.com/rust-lang/rust/issues/150969) ICE: valtrees: 'called `Result::unwrap()` on an `Err` value: ReferencesError(ErrorGuaranteed(()))'
- [#149588](https://github.com/rust-lang/rust/issues/149588) layout errors in transmute checking don't get emitted
- [#154780](https://github.com/rust-lang/rust/issues/154780) [ICE]: delegation: `TyKind::Error constructed but no error reported`

<a id="15"></a>
## 15. Each encoded record is written once and hashed after it is final

**Statement.** The metadata encoder writes each dep node at most once under concurrent execution, and crate_hash is not computed before metadata encoding finishes

**Check.** With mirth, log (table, key) and (dep node) writes and fail on any duplicate. Log when crate_hash and the svh are computed relative to the encoder finishing, and fail if hashing happens first.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** metadata / incremental.

**Tested today?** There are a few assertions in the dep-graph encoder. The ordering of hashing relative to encoding is not checked.

**mirth.** Excellent. It uses the same write hooks as the reader-writer property.

**Bugs that violated it:**

- [#150018](https://github.com/rust-lang/rust/issues/150018) assertion failed: trying to encode a dep node twice
- [#142778](https://github.com/rust-lang/rust/issues/142778) ICE: rustc_query_system: dep_graph: assertion failed (dep node index out of range)
- [#163426](https://github.com/rust-lang/rust/issues/163426) [ICE]: rustdoc ICE with -Zmetrics-dir: crate_hash(LOCAL_CRATE) called before metadata encoding

<a id="16"></a>
## 16. #[expect] is fulfilled exactly when the lint fires

**Statement.** #[expect(L)] fulfillment status is not always correctly aligned with whether #[warn(L)] would actually emit L. Cases involving match guards, derives, and cfg_attr show mismatches where expectations are reported as unfulfilled despite the lint firing, or vice versa.

**Check.** For each #[allow] or #[expect] in the corpus, generate the #[warn] and #[expect] variants. Build both and check the pairing: lint emitted exactly when the expectation is fulfilled.

**Cost:** moderate (one build per attribute; batchable). **Kind:** differential. **Subsystem:** lints.

**Tested today?** There are UI tests for specific cases. No automatic pairing check exists.

**mirth.** Low. A source-rewrite harness is enough.

**Bugs that violated it:**

- [#152004](https://github.com/rust-lang/rust/issues/152004) Regression on nightly: unused pub(crate) use::*; is not actually unused
- [#151983](https://github.com/rust-lang/rust/issues/151983) Missing "unused variable" warning when using a match guard
- [#152401](https://github.com/rust-lang/rust/issues/152401) unfulfilled-lint-expectations for missing_docs

<a id="17"></a>
## 17. Metadata reads hit entries the writer wrote

**Statement.** Reads of metadata table entries fail when the upstream encoder never wrote those entries, causing ICEs when dependent crates attempt to access missing (table, DefIndex) pairs or when compiler passes try to read unwritten entries from internal lookup tables

**Check.** With mirth, log every table write (table, index) in the writer process and every lookup in reader processes across the whole cargo build. Join the logs. A read of an unwritten entry is a bug, except for tables documented as default-on-absent, which need an allowlist.

**Cost:** moderate (whole-build logs). **Kind:** invariant. **Subsystem:** metadata.

**Tested today?** Not tested. Missing entries usually decode as defaults with no error.

**mirth.** Excellent. It needs both writer and reader instrumentation across one cargo build, which no other tool can do.

**Bugs that violated it:**

- [#163426](https://github.com/rust-lang/rust/issues/163426) [ICE]: rustdoc ICE with -Zmetrics-dir: crate_hash(LOCAL_CRATE) called before metadata encoding
- [#159233](https://github.com/rust-lang/rust/issues/159233) [ICE]: resolve: `no entry found for key`

<a id="18"></a>
## 18. Query results contain no inference variables

**Statement.** Inference variables leak into the const literal lowering logic (lit_to_const), appearing in cached query results where they cause hashing panics.

**Check.** With mirth, at every query return and every HashStable call, check has_infer() and has_placeholders() on the value and record the query and key on failure.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** query system / type inference.

**Tested today?** There are a few debug assertions at specific sites. No global check exists.

**mirth.** Excellent. One generic hook at the query provider return.

**Bugs that violated it:**

- [#153525](https://github.com/rust-lang/rust/issues/153525) [ICE]: `type variables should not be hashed`
- [#153524](https://github.com/rust-lang/rust/issues/153524) [ICE]: `const variables should not be hashed`

<a id="19"></a>
## 19. Machine-applicable suggestions apply cleanly

**Statement.** Applying every MachineApplicable suggestion (rustfix) gives code that compiles and the diagnostic is gone, provided that suggestions are generated with correctly-calculated non-empty, non-overlapping spans. Violations occur when the suggestion system generates suggestions with malformed spans that cause compiler panics.

**Check.** Run cargo fix --broken-code on corpus crates with all warn-by-default lints enabled, then rebuild. Check that it compiles and that the original lint no longer fires.

**Cost:** moderate. **Kind:** differential. **Subsystem:** diagnostics / lints.

**Tested today?** run-rustfix UI tests cover curated cases.

**mirth.** Poor. It is a cargo-level differential.

**Bugs that violated it:**

- [#161213](https://github.com/rust-lang/rust/issues/161213) invalid range in macro triggers compiler panic at assertion `left == right` failed: suggestion must not have overlapping parts
- [#161472](https://github.com/rust-lang/rust/issues/161472) [ICE]: Macro captures a list meta item with $m:meta and passes it directly to #[derive] causes `must not be empty and have no suggestion`

<a id="20"></a>
## 20. Polonius accepts at least what NLL accepts, and no more than is sound

**Statement.** -Zpolonius=next has soundness bugs in opaque type region handling that allow acceptance of programs with undefined behavior, violating the guarantee that Polonius-only-accepted programs should be sound under Miri.

**Check.** Borrow-check the corpus with both. Any NLL-accept/Polonius-reject is a bug. For Polonius-only accepts, which mostly come from the UI suite, run under Miri.

**Cost:** moderate. **Kind:** differential. **Subsystem:** borrowck.

**Tested today?** There is a polonius compare-mode on some UI tests.

**mirth.** Low. It is an accept/reject differential.

**Bugs that violated it:**

- [#153215](https://github.com/rust-lang/rust/issues/153215) free region visitor for liveness marking regions dead and polonius alpha soundness
- [#160669](https://github.com/rust-lang/rust/issues/160669) Zpolonius=next soundness bug: defining use of an opaque type discards the first one's region

<a id="21"></a>
## 21. Unstable syntax is gated before expansion

**Statement.** Every unstable syntactic form must be rejected at pre-expansion on stable without its feature gate, even inside #[cfg(FALSE)] blocks or unused macro_rules arms, to prevent code breakage when feature gates are removed.

**Check.** Take each feature-gate UI test that exercises syntax, wrap the gated syntax in #[cfg(FALSE)], and compile without the feature. The compile must fail.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** parser / feature gates.

**Tested today?** Some features have cfg(FALSE) tests. It is not enforced for each one.

**mirth.** Poor. It is a source-rewrite harness.

**Bugs that violated it:**

- [#152501](https://github.com/rust-lang/rust/issues/152501) `try bikeshed $ty { … }` is not pre-expansion gated (affects beta+nightly)
- [#152499](https://github.com/rust-lang/rust/issues/152499) Inline const patterns are no longer pre-expansion gated

<a id="22"></a>
## 22. P5+ metadata determinism under irrelevant perturbation

**Statement.** .rmeta contents must not depend on unrelated rlib files present in the library search path (-L), i.e., two builds with identical source, flags, and target produce byte-identical .rmeta even when the set of available (but unused) libraries in the search path differs.

**Check.** For every crate in a cargo build, run the build twice from clean. Between the runs, change one perturbation: add a decoy rlib to the search path, use a different target dir, set junk env vars, or shift the clock with faketime. Byte-compare every output. When they differ, use mirth's table-write recording to find the first table or row that diverges, then the query that produced it.

**Cost:** cheap (2 builds; perturbations are free). **Kind:** differential. **Subsystem:** metadata / const-eval / whole-compiler.

**Tested today?** rustc has tests/run-make reproducibility tests on a few small fixtures and one perturbation each. mirth already checks P5 on whole cargo builds without perturbation. Search-path and build-dir perturbations across real crates are not tested.

**mirth.** Excellent. P5 already exists. Adding perturbations is a harness change. mirth's per-table write log turns a byte diff into the table, the row and the query that wrote it.

**Bugs that violated it:**

- [#159677](https://github.com/rust-lang/rust/issues/159677) `.rmeta` contents depend on unrelated files in the library search path

<a id="23"></a>
## 23. P4+ no query reads untracked state

**Statement.** No query reads HashMap/HashSet iteration order (including symbol interner order) in ways that affect incremental compilation artifacts (like .rmeta) unless the dependency on which symbols get interned/which HashMap entries are iterated is recorded in the dep graph.

**Check.** Use mirth instrumentation of env::var*, SystemTime/Instant, fs::metadata, and iteration of std/Fx HashMaps. Attribute every read to the query on top of the stack and flag reads that are not covered by a tracking query. Confirm a finding by perturbing that input and checking whether the query's fingerprint changes.

**Cost:** cheap once instrumented. **Kind:** invariant. **Subsystem:** incremental / query system.

**Tested today?** Upstream has a lint against unordered iteration (rustc::potential_query_instability) with many allow exceptions. mirth's P4 covers metadata encoding only.

**mirth.** Excellent. This is mirth's core use case: extend the P4 hooks from encode_metadata to every query frame.

**Bugs that violated it:**

- [#159677](https://github.com/rust-lang/rust/issues/159677) `.rmeta` contents depend on unrelated files in the library search path

<a id="24"></a>
## 24. Symbol names are injective and stable

**Statement.** The v0 and legacy symbol mangling schemes must encode all type attributes that the type system considers semantically distinct, including splat on function types, to ensure monomorphized instances never share symbol names. V0 symbols must round-trip through rustc-demangle to the instance's path."

**Check.** With mirth, record (Instance, symbol_name) pairs in every codegen process of a cargo build. Check that the mapping is injective across crates and round-trip each v0 symbol through rustc-demangle against the printed instance.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** symbol mangling.

**Tested today?** Mangling UI tests cover specific types. There is no whole-build injectivity check.

**mirth.** Good. Hook symbol_name, which is easy, and join the results across crates.

**Bugs that violated it:**

- [#158644](https://github.com/rust-lang/rust/issues/158644) Splat is ignored in symbol mangling, leading to symbol clashes

<a id="25"></a>
## 25. dyn types have a dyn-compatible principal

**Statement.** In successful compilations, every TyKind::Dynamic that reaches typeck, MIR, or metadata must have a principal trait that is explicitly marked dyn-compatible; allowing Dynamic types with non-dyn-compatible principals (like DerefPure) enables unsound behavior.

**Check.** With mirth, when a Dynamic type is interned, record its principal. At the end of a successful session, assert is_dyn_compatible for each one.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** trait-system.

**Tested today?** Dyn-compatibility is checked at direct dyn-syntax sites. No global check exists.

**mirth.** Good. One intern hook plus a check at session end.

**Bugs that violated it:**

- [#154619](https://github.com/rust-lang/rust/issues/154619) `deref_patterns` is unsound due to `dyn` of subtrait of `DerefPure`

<a id="26"></a>
## 26. Built-in attributes reject malformed arguments

**Statement.** Every built-in attribute rejects argument forms its template does not allow by producing a diagnostic error, rather than silently ignoring excess or incorrectly-formed arguments.

**Check.** From the BUILTIN_ATTRIBUTES templates, generate malformed forms for each attribute (extra arguments, list instead of word, name-value instead of word) and assert that a diagnostic is produced.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** attribute parsing.

**Tested today?** There are per-attribute tests, but not generated from the templates.

**mirth.** Poor.

**Bugs that violated it:**

- [#154977](https://github.com/rust-lang/rust/issues/154977) Invalid value accepted for `#[macro_export(local_inner_macros)]`

<a id="27"></a>
## 27. Advertised target features exist in LLVM

**Statement.** Every target feature that rustc lists for a target, and every feature that an asm register class or intrinsic requires, must be recognized by LLVM and must be consistent with the target's baseline—that is, register class and intrinsic feature requirements cannot exceed what is available for that target's baseline configuration.

**Check.** For every target, enumerate rustc's feature table and query LLVM's feature list. Compile an empty function with each feature enabled and confirm there is no 'unknown feature' warning or spurious requirement.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** codegen / targets.

**Tested today?** There is a partial tidy-style check.

**mirth.** Poor.

**Caveat.** The check judged this not general: The single cited issue provides clear evidence of a violation. Register classes were requiring features in a manner inconsistent with the target's baseline capabilities. The fix involved both adjusting feature requirements and adding missing implied features, confirming the underlying inconsistency that the property prohibits.

**Bugs that violated it:**

- [#159976](https://github.com/rust-lang/rust/issues/159976) `armv8r-none-eabihf` target cannot use FPU instructions in inline asm

<a id="28"></a>
## 28. Eq and Hash agree for std types

**Statement.** For every std type implementing both Eq and Hash, a == b implies hash(a) == hash(b), for every Hasher. (The property statement is precise and matches the evidence; the violation in Path on Windows with verbatim paths is a concrete instance of this general invariant being broken.)

**Check.** Run property tests over generated values of std types, including borrowed forms through Borrow, with several hashers.

**Cost:** cheap. **Kind:** semantic. **Subsystem:** library.

**Tested today?** There are ad hoc tests only.

**mirth.** None. It is a library property test.

**Bugs that violated it:**

- [#161651](https://github.com/rust-lang/rust/issues/161651) Eq returns true but hashes are different for some verbatim paths on Windows

<a id="29"></a>
## 29. Each diagnostic is emitted once

**Statement.** When the same diagnostic (identified by identical level, error code, message text, and primary source span) is emitted more than once in a single compilation session, it appears multiple times in --error-format=json output. This can be detected by extracting diagnostics from JSON and checking for exact duplicates by the tuple (level, code, message, span).

**Check.** Post-process JSON diagnostics from corpus builds and from the UI suite run with --error-format=json, and look for duplicate keys.

**Cost:** cheap. **Kind:** invariant. **Subsystem:** diagnostics.

**Tested today?** The emitter deduplicates some cases. UI .stderr snapshots would show duplicates, but nothing asserts they are absent.

**mirth.** Poor. It is an output check.

**Bugs that violated it:**

- none of the cited issues held up



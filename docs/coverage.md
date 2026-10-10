# Which parts of the compiler run

`fixtures/sink` covers Rust's grammar ([`grammar.md`](grammar.md)); this measures how much of
the compiler compiling it reaches. mirth-watch's `[coverage]` mode (`rustc/coverage.toml`)
instruments every function and closure of the compiler's own crates (`rustc_*`) with one call
at entry, which records the function's first call in each process; at exit each rustc
process writes the functions it entered. `mirth-lab coverage` joins those with the site
tables.

    MIRTH_RUST=<rust checkout> BUILD_DIR=<dir> MIRTH_WATCH=rustc/coverage.toml rustc/build.sh
    MIRTH_OUT=<logs> RUSTC=<dir>/<host>/stage1/bin/rustc cargo build   # any number of builds
    target/release/mirth-lab coverage --sites <dir>/mirth-sites --logs <logs> [--files] [--unhit rustc_borrowck]

The instrumented compiler is the same source as `rustc-verify12` (pinned nightly plus the
local patches); 72,774 functions in 80 crates.

## Sink: a clean build, an incremental rebuild after an edit, and a run

27 rustc processes; **26454 of 71649 functions ran (36.9%)**. The largest crates:

| crate | ran | functions | |
|---|---:|---:|---:|
| `rustc_middle` | 5002 | 10787 | 46% |
| `rustc_query_impl` | 1423 | 4576 | 31% |
| `rustc_ast` | 1445 | 2846 | 51% |
| `rustc_type_ir` | 1293 | 2750 | 47% |
| `rustc_hir_typeck` | 793 | 2559 | 31% |
| `rustc_trait_selection` | 424 | 2533 | 17% |
| `rustc_target` | 606 | 2213 | 27% |
| `rustc_public` | 0 | 2176 | 0% |
| `rustc_borrowck` | 582 | 2167 | 27% |
| `rustc_hir_analysis` | 739 | 2108 | 35% |
| `rustc_mir_transform` | 711 | 2101 | 34% |
| `rustc_lint` | 759 | 1973 | 38% |
| `rustc_resolve` | 785 | 1876 | 42% |
| `rustc_session` | 528 | 1785 | 30% |
| `rustc_codegen_llvm` | 669 | 1774 | 38% |
| `rustc_parse` | 723 | 1745 | 41% |
| `rustc_codegen_ssa` | 597 | 1719 | 35% |
| `rustc_attr_parsing` | 235 | 1448 | 16% |
| `rustc_span` | 667 | 1265 | 53% |
| `rustc_metadata` | 731 | 1232 | 59% |

Not run at all: `rustc_public` (stable MIR, for tools), `rustc_sanitizers`, `rustc_transmute`
(`TransmuteFrom` checks), `rustc_thread_pool` (only with `-Zthreads`), the proc-macro crates
(`rustc_macros` and others, which run while the compiler itself is built), `rustc_graphviz`
(MIR and dep-graph dumps).

**Errors.** Sink compiles without errors, so error reporting never runs. Of the 45,195
functions that never ran, at least 12,651 (28%) are diagnostics or error paths by name or file
(`report`, `error`, `suggest`, `lint`, `emit_`, …); in `rustc_trait_selection`,
`rustc_borrowck` and `rustc_hir_typeck` more than half. More syntax will not reach those;
programs that fail to compile, built incrementally, will.

## With option configurations

`mirth-lab coverage-flags` builds sink once per row of a pairwise transitions table (clean with
the row's A options, then rebuilt after one random edit with its B options). 60 rows, 1,586
rustc processes in all with the run above: **30,657 functions (42.8%)**. The largest gains: MIR
optimization passes (+460, from `-Copt-level` and `-Zmir-opt-level`), `rustc_trait_selection`
(+409, from `-Znext-solver` and edits that fail to compile), `rustc_session` (+351, option
handling), `rustc_thread_pool` (+226, `-Zthreads`).

## rustc's UI tests

`mirth-lab ui-coverage run` compiles each UI test file (the way its `//@` headers say) with the
instrumented compiler and keeps the functions it reaches beyond sink's; `pick` chooses tests
greedily. Of 20,419 files, 1,839 were skipped (auxiliary crates, other targets); together the rest
reach **17,668 functions sink does not (61% of the compiler with sink's)**; **300 picked tests
reach 13,626 of them (55%)**. The first few:

| test | adds |
|---|---:|
| `parallel-rustc/generic-const-exprs-deadlock-issue-120757.rs` | 1,249 |
| `layout/uninitialized-gat-projection-cycle-issue-153205.rs` | 811 |
| `self-profile/pretty_print_no_ice.rs` | 504 |
| `attributes/malformed-attrs.rs` | 444 |
| `abi/stack-protector.rs` | 432 |

`mirth-lab ui-fuzz` runs the picked tests through incremental rebuilds after the fuzzer's edits,
each compared with a clean build (status, diagnostics, outputs): error reporting and recovery
under incremental compilation, which sink cannot reach.

Running every UI test outside compiletest also showed that compiletest pins the old trait
solver, while nightly defaults to the new one: [`solver.md`](solver.md).

### Through incremental rebuilds

`mirth-lab ui-fuzz` over every UI test that compiles standalone (18,553 files), up to 8 random
edits each: **123,063 edit, incremental rebuild and clean rebuild cycles**, each compared on exit
status, diagnostics and outputs. Checked first on finding 7's shape (a warning from inline
assembly, lost when a codegen unit is reused), which it finds in 2 of 37 edits.

New: finding 17 (the `-Zunleash-the-miri-inside-of-you` warning is lost on a rebuild) and
finding 18 (after a fatal error, a rebuild reports fewer errors than a clean build); both are
diagnostics only, and both are recognized as known since. Nothing else differed: no rebuild
accepted what a clean build rejected or the other way round, and no output differed.

## What can run at all: the call graph

Coverage needs a denominator: functions that no execution can reach should not count against
the corpus. mirth-watch's `callgraph` diagnostic (`rustc/callgraph.toml`) writes, for each body
of the compiler's crates (and rustdoc's, a second entry point into them: `WITH_RUSTDOC=1`), its
outgoing edges, with functions named by `DefPathHash` (the same from every crate; paths
printed through re-exports did not match), to `<crate>-<stable crate id>.graph` (two versions
of rustc-hash share a name):

- direct calls; for a call through a trait, the trait item, the closure or function item a
  call through `Fn*` names, and the implementation the caller's types resolve it to, where
  they do;
- functions and closures used as values, in the body and in its promoted constants
  (`&[f, g]`, `&(f as fn())`), and the implementation `<T as Debug>::fmt` as a value resolves to;
- callees MIR inlining merged into the body (mirth sees MIR after inlining);
- constants' and statics' initializers (`mir_for_ctfe`), for tables of function pointers;
- **types built** (`construct`): aggregates, constructors, constants of a struct or enum
  type, and the types of locals and of calls' generic arguments;
- **trait demands** (`demand <trait> <type>`): what the body needs implemented. A call's
  bounds instantiated with its arguments (`HashMap::insert` with `K = DefId` needs
  `DefId: Hash`), with supertraits; the bounds of the impl a call or constant resolves to
  (`impl<A: Step> Iterator for Range<A>`); casts to `dyn Trait` (vtables); and, for each
  bound, after normalizing it (`<Op as TypeOp>::ErrorInfo: ToUniverseInfo`), the bounds of
  the impl that proves it, selected in the body's own environment (a blanket impl's
  `T: From<U>`, a derive's `T: Encodable<E>` for every field) and of the trait's associated
  types (`type Domain: JoinSemiLattice`), recursively.

`mirth-lab callgraph` computes reachability from the compiler's `main`s, every function with a
foreign ABI (callbacks from C, C++ and LLVM), every implementation of a trait from outside the
compiler (which `std` may call), and every initializer. A call to a trait item reaches its
implementations (class hierarchy analysis), but an implementation for one of the compiler's
structs or enums counts only once reachable code **builds the type** (for methods taking
`self`; rapid type analysis) and **demands the trait for it**. Exceptions, where bounds do not
say what runs: `Drop` (drop glue), impls taking part in specialization and the traits a
specializing impl's bounds name (`SpecIntoSelfProfilingString`), and crates.io crates the
scope takes in by name (rustc-hash: other dependencies, outside the graph, name its types).

Programs outside the compiler that link it (`tests/ui-fulldeps`, the compiler crates' unit
tests) call it from their own `main`s: what they ran is a root (`--external`).
`--why <function>` prints the chain that reaches a function and what released each gate.

**The check:** every function that coverage saw run must be reachable. The first graph left
11,279 such functions out (re-exported paths); each fix above cut the number to 0, and each
refinement since was checked against it (rapid type analysis first left 1,945 out, unit structs
and statics; trait demands 516, then 231, 15, 6, 1: impl bounds, normalization, item bounds,
inlined calls, generic selection, specialization, rustc-hash).

**Result:** of 71,257 functions, **8,741 (12.3%) cannot run**; 62,516 can. The proc-macro
crates, which run when the compiler is built, are left out.

**Only on a compiler bug.** A body no path of which returns, and each of whose paths ends in a
panic (`bug!`, `span_bug!`, `unreachable!`, a failed `unwrap`), directly or through such a body
(`default_extern_query`, and the extern-provider closures that call it), runs only when the
compiler has a bug: 911 of the reachable functions. A body that ends in a fatal error instead is
a normal error path and is not counted. Coverage is reported with and without them.

## rustc's own test suites

`rustc/coverage-suites.sh` runs a suite through compiletest (`./x.py test`) on the
instrumented compiler, with `MIRTH_OUT` set, folding logs as they finish
(`mirth-lab coverage-compact`): compiletest handles what a standalone runner cannot (auxiliary
crates, every revision, `minicore` cross-targets, run-make). With `--keep-stage 0 --keep-stage 1`,
and a refusal when the log shows the compiler compiling: a changed mirth runtime once made
`x.py` rebuild the compiler without the instrumentation, so `build.sh` now keeps a runtime per
build directory and replaces it only when it changes. `WRAPPED=1` runs the compiler crates'
unit tests instead, compiled through mirth-watch (that run rebuilds the stage 1 sysroot
without `std`: build `library` again after it). `rustc/coverage-fulldeps.sh` runs the
`ui-fulldeps` tests compiletest skips at stage 1, compiled against the stage 1 crates by hand.

| suite | tests passed | functions reached |
|---|---:|---:|
| ui | 22,258 | 46,398 |
| ui, without compiletest's `-Znext-solver=coherence` | 20,915 (1,343 fail) | 46,119 |
| ui, with the profiler runtime (`build.profiler`) | 22,261 | 46,404 |
| run-make | 420 (2 fail) | 33,448 |
| incremental | 180 | 29,842 |
| codegen-llvm | 1,122 | 28,045 |
| crashes | 172 | 27,711 |
| mir-opt | 413 | 26,966 |
| rustdoc-html | 804 | 26,176 |
| rustdoc-ui | 461 | 25,996 |
| pretty | 114 | 24,428 |
| coverage | 233 | 24,268 |
| assembly-llvm | 738 | 23,913 |
| codegen-units | 46 | 23,826 |
| debuginfo (gdb) | 134 | 23,514 |
| incremental, with `RUSTC_VERIFY_REUSE=1` (every reused query recomputed and compared) | 179 (1 fail) | 30,247 |
| ui-fulldeps, stage 1 skips by hand | 38 (13 fail) | 22,553 |
| coverage-run-rustdoc | 1 | 19,833 |
| rustdoc-json | 187 | 17,839 |
| compiler crates' unit tests | 1,479 | 7,023 |
| ui-fulldeps (compiletest, stage 1) | 30 | 2,815 |

(The run without the solver pin uses a local compiletest switch, `COMPILETEST_NO_SOLVER_PIN`,
[`hunt/compiletest-solver-pin.patch`](hunt/compiletest-solver-pin.patch). The incremental
failure under `RUSTC_VERIFY_REUSE` is the check's own: recomputing a reused query emits its
lint a second time, `warnings-reemitted.rs` sees the warning twice.)

## Runs the suites hardly make

`rustc/coverage-run.sh <name> <command>` runs any command with `MIRTH_OUT` set and folds the
logs the same way.

| run | functions reached |
|---|---:|
| ui tests through incremental rebuilds (`mirth-lab ui-fuzz`, 18,553 tests, 3 edits each) | 44,196 |
| `mirth-lab coverage-generators`: every `--print` request on the host and on all 334 targets; minicore and an ABI file compiled for every target at `-Copt-level=0` and 3; the 300 picked UI tests under 48 debugging and printing options | 41,768 |
| `mirth-lab coverage-generators --only links`: a binary, cdylib, staticlib and dylib on minicore for every target with `-Clinker=true`, under 11 sets of linker options | 18,228 |

**All together, with sink and its option configurations: 50,762 of the 62,516 functions that
can run (81.2%); without the 911 that only panic, 50,715 of 61,605 (82.3%).**
`rustc/coverage-report.sh` recomputes this; `mirth-lab callgraph --gaps <file>` lists the rest
by crate and file, largest first. What is left, and the plan for it: [coverage-handoff.md](coverage-handoff.md).

## The checks of `checks.md` as coverage suites (2026-10-10)

[`rustc/coverage-checks.sh`](../rustc/coverage-checks.sh) runs each check that takes a `--rustc`
once through the block-instrumented compiler (`build-blk`), each as its own suite
(`cov-suites/check-<name>`), at `JOBS=3` (at 8 jobs the log compactor fell 26 GB behind on
solver-diff, so that suite covers part of the corpus). With the two coverage-guided fuzzing
suites, the 13 new suites take reachable blocks (panic-only blocks aside) from 562,651 of
696,588 (80.8%) to 566,335 (81.3%), and functions from 50,771 to 50,858 of 62,523 (81.2% to
81.3%). Blocks each suite adds on its own, over the 48 suites that existed before:

| suite | blocks added |
|---|---:|
| guided gate-mutate (150 min) | 2,856 |
| unguided gate-mutate (150 min, same seed) | 1,093 |
| gate-mutate (20,000 mutants) | 905 |
| rewrite-diff | 380 |
| suggest-diff | 211 |
| lint-check | 158 |
| solver-diff (partial) | 155 |
| gate-check | 146 |
| opt-diff | 111 |
| diag-check | 94 |
| repro-diff | 66 |
| abi-diff (all targets, one seed) | 26 |
| scale-check | 24 |

Counted against everything that had run before, guidance pays: the guided fuzzer adds 2.6× the
blocks of the unguided one with the same budget (against the smaller snapshot it started from,
the two looked level; [`checks.md`](checks.md)). The oracle checks reuse the UI corpus and add
little coverage; their value is the properties they check, not new code reached.

## Beyond functions and blocks

The no-rebuild parts of [coverage-plan.md](coverage-plan.md) (M1, M3, and the codegen half of
M5), measured 2026-10-10 against the block build (`build-blk`), its runs (`cov-blk`) and the
suites folded into `cov-suites` since (the third-batch checks' suites were still being folded
in), and the block report's unreachable list (`cov-blk/gaps.json`). `rustc/coverage-report.sh` reruns all of them after the function and block
report; each writes `coverage-<name>.txt` and its gap lists next to `gaps.md`.

Where a number comes from block sites rather than its own instrumentation, it is an estimate:
a span "ran" when a block site starting inside it was reached. Files whose source has moved
since the coverage build are detected (the site tables' snippets no longer match the text) and
skipped; three were, all changed after the build.

### Static denominators (`mirth-lab coverage-static`, M1)

Against the block corpus (`cov-blk/suites`, its sink and option-configuration logs) together with
the suites folded into `cov-suites` since, of which the checks' block runs are the new ones:
587,602 of the 716,826 sites in reachable functions ran.

| dimension | denominator | measured today | gap list |
|---|---|---|---|
| branch arms (source level) | 53,721 decisions, 136,428 arms; 115,951 arms in reachable functions (41,265 of all arms are implicit: an `if` without `else`, `?`'s return, a loop's exit) | 67,444 arms have a block site in their body; 984 of those are panic-only; 57,957 taken (85.9%). Implicit arms wait for M2 | `gaps-arms.md` (9,487 arms) |
| configuration branches | decisions in reachable functions whose condition reads a feature gate (321), an option (334), a target property (357) or the edition (102): 1,114 (decision, kind) pairs over 321 names | 1,078 consulted by a measured run; both arms seen at 168 | `gaps-config.md` (853 decisions seen one way) |
| feature gates | 262 unstable features: 136 checked by accessor calls (356 call sites), 109 only through generic code that names them (`sym::`, `Features::`, an attribute parser's `unstable!`), 17 not named in the compiler that way | all 136 consulted at least once (on or off: M4 records which) | `gaps-features.md` |
| delayed-bug sites | 242 `span_delayed_bug`/`delayed_bug` calls; 238 in reachable functions | 177 reached (74.4%) | `gaps-delayed.md` (61) |
| keyed engine sites | 330 queries (38 `eval_always`, 58 cached on disk, 26 feedable) × 1,018 engine sites in reachable functions (`rustc_query_impl::execution`, `rustc_middle::dep_graph::graph`; 31 on the load-from-disk path, 66 on the feeding path) | 307,444 possible (query, site) pairs, fewer once each query's path is taken into account: M4 measures them | — |
| metadata tables | 82 (`define_tables!`: 21 defaulted, 61 optional) | see below | `gaps-rmeta.md` |

By kind, the arms with a body that were taken: `match` 79.7%, `if` 91.6%, loops 93.8%,
`let … else` 54.4% (the `else` is usually the error path), `&&`/`||` 95.5%. The largest gap
lists are `rustc_codegen_ssa` (956 arms), `rustc_target` (668, most of them per-target tables),
`rustc_middle` (649), `rustc_codegen_llvm` (642) and `rustc_const_eval` (599). The delayed-bug
sites never reached are led by `rustc_hir_analysis` (17), `rustc_trait_selection` (7) and
`rustc_hir_typeck` (7).

Configuration decisions consulted but seen only one way include `unsized_fn_params` (10
decisions, none seen with the other arm), `specialization` (9), `min_specialization` (7),
`coroutines`, `const_trait_impl`, `async_fn_track_caller` (6 each), `yield_expr` and
`unboxed_closures`: places where a gate-mutate mutant with the gate on (or off) would take the
other arm. For the edition, 102 decisions, 24 seen both ways.

### Diagnostics (`mirth-lab diag-coverage`, M3)

`--collect` compiled each of the 18,624 standalone UI tests with `--error-format=json`
(`--emit=metadata`, the test's own flags, `~/mirth-work/cov-static/diags`); the report folds
the result against what exists.

| dimension | denominator | emitted |
|---|---|---|
| error codes | 518 documented, 131 marked no longer emitted | 365 of the 387 live ones (94.3%); four retired codes still emitted: E0477, E0516, E0518, E0718 |
| lints | 253 in `rustc -W help` (62 allow, 140 warn, 51 deny by default) | 225 (88.9%): allow 54, warn 128, deny 43 |
| diagnostic messages | 1,920 `#[diag("…")]` messages of `#[derive(Diagnostic)]` items and variants; 1,900 not just an argument | 1,308 (68.8%) |
| subdiagnostic messages | 721 `#[note]`/`#[help]`/`#[label]`/`#[suggestion]` messages of `#[derive(Subdiagnostic)]` items; 705 measurable | 599 (85.0%) |
| suggestions | per applicability | MachineApplicable 10,901, MaybeIncorrect 6,051, HasPlaceholders 1,943, Unspecified 149; 72 lints offered a machine-applicable fix |

A lint whose diagnostic has an error code (E0133 for `unsafe_op_in_unsafe_fn`) names the lint
only in its note (`` `#[warn(…)]` on by default ``); the report reads those notes too. The gaps
(`gaps-diagnostics.md`, `gaps-diag-structs.md`) are mostly what this corpus cannot reach by
construction: crate-loading errors (E0460–E0464, E0514, E0519) and lints that need auxiliary
crates (`proc_macro_derive_resolution_fallback`, `legacy_derive_helpers`,
`unused_crate_dependencies`), code generation or linking (`large_assignments`, `linker_messages`,
`linker_info`), or another target (`uses_power_alignment`, `x86_softfloat_sse`,
`aarch64_softfloat_neon`). The diagnostic messages never emitted are led by
`rustc_codegen_ssa` (136), `rustc_attr_parsing` (55), `rustc_session` (47) and
`rustc_metadata` (46).

### Metadata tables (`mirth-lab rmeta-coverage`, M3)

From the blessed records of `fixtures/chain` and `fixtures/wide` (`tests/rmeta/*.txt`, clean
and touch builds): a table is encoded when a process's `encoded` section names a write to it,
decoded when a process runs an extern query whose provider (`cstore_impl.rs`'s `provide!`,
followed through the decoder methods it calls) reads it.

| crate type | encoded | decoded |
|---|---:|---:|
| bin | 0 | 58 |
| lib | 67 | 59 |
| proc-macro | 9 | 50 |
| any | 67 of 82 | 62 of 82 |

Never encoded by these fixtures: the stability tables (`lookup_stability`,
`lookup_const_stability`, `lookup_default_body_stability`), `intrinsic`, `const_param_default`,
`thir_abstract_const`, `impl_parent`, `coerce_unsized_info`, `adt_destructor`,
`adt_async_destructor`, `default_fields`, `deduced_param_attrs`,
`rendered_precise_capturing_args`, `explicit_implied_const_bounds`,
`live_args_for_alias_from_outlives_bounds`. No record has a dylib, staticlib or cdylib. Five
tables are read by no extern query (the decoder reads them directly or at crate load).

### Codegen and target (`mirth-lab codegen-coverage`, M5 half)

`--collect` compiled the 3,493 runnable UI tests to LLVM IR at `-Copt-level=0` and 3
(`~/mirth-work/cov-static/ir`; 56 did not build in one of the two).

| dimension | denominator | reached |
|---|---|---|
| LLVM intrinsics | 76 `llvm.*` intrinsic names in rustc_codegen_llvm (8 more are metadata names and special globals) | 59 in the IR (77.6%); 80 distinct intrinsics in the IR overall, LLVM's own included |
| Rust intrinsics with their own codegen arm | 195 `sym::` arms in the codegen intrinsic matches; 189 measurable | 143 lowered by a measured suite (75.7%); not: the f16 and f128 math, `round*`, `maximum*`/`minimum*`, atomic min/max, the SVE tuple intrinsics, `autodiff`, `offload`, `simd_carryless_mul` |
| calling-convention lowering | 26 `rustc_target/src/callconv/<arch>.rs` files, 4,432 block sites | 3,705 reached (83.6%; the generators suite compiles for every target); least: `aarch64.rs` 63.5%, `mod.rs` 67.3%, `msp430.rs` 69.8%, `nvptx64.rs` 71.6% |
| conventions and linkage in the IR | — | `fastcc` in nearly every test; `x86_64_sysvcc` 5, `win64cc` 2, `tailcc` 2, `x86_vectorcallcc` 1, `preserve_nonecc` 1; external, internal, private and weak linkage; 13 target features enabled somewhere |

The gap lists are in `gaps-codegen.md`.
## Beyond blocks: in-compiler dimensions (`build-cov2`)

[`coverage-plan.md`](coverage-plan.md) names two blind spots of block coverage: a block reached
from several predecessors does not say which way a switch went, and a generic body (the query
engine, the dependency graph) is one site for every query that runs through it. A second
instrumented compiler measures the dimensions that close them
(`rustc/build-dims.sh`: `rustc/coverage-dims.toml` and
[`hunt/coverage-dims.patch`](hunt/coverage-dims.patch) on the campaign's patch series, built
into `build-cov2` from a separate source tree). It records everything `build-blk` does, with the
same site identities, plus:

| dimension | how | site table | per process |
|---|---|---|---|
| branch arms | mirth-watch splits each `SwitchInt` edge into a block other paths reach too, with a `cover` call | 61,020 `arm` rows (58,903 with a site of their own) | `V` lines, like blocks |
| configuration arms | the switch's discriminant traced back (copies, casts, negations, comparisons) to a `Features::<gate>()` accessor, a `read_<option>()` getter, an edition check, a `Target`/`TargetOptions` field or a session query; every arm of such a switch is listed (an arm into a block only it reaches is that block's site) | 2,563 arms tagged | none |
| keyed engine paths | `[[coverage.keyed]]`: the query engine and the per-query plumbing keyed by the vtable's `dep_kind`, read at entry; each block reports `site ^ mix(key)` | 7,070 `keyed` rows, 1,247 `keyenum` (variant names) | `K` lines, about 8k |
| incremental transitions | the dependency graph keyed by the `DepNode`'s kind | (in the above) | |
| feature gates consulted | each `Features::<gate>()` accessor keyed by its return value | (in the above) | |
| type kinds | layout, ABI, coercion, const eval and `ty::util` keyed by the `TyKind` of their first `Ty` argument, read through the interned pointer | (in the above) | |
| call pairs | at each function's entry, the instrumented function running on the thread (a thread-local set at entry, restored at returns) | none | `D` lines, about 30k |
| MIR pass effect | `RUSTC_PASS_EFFECT`: each body's blocks and locals hashed (spans aside) before and after each pass | none | a `.passes` file |
| lock contention | `RUSTC_LOCK_CONTENTION`: `Lock::lock` tries first and records the `#[track_caller]` site it found held | none | a `.locks` file under `-Zthreads` |

Cost: about twice `build-blk`'s compile time (an iterator-chain stress test: 25 s against 12.6 s
user time; `build-blk` is itself slower than a plain compiler) and 130 MB more memory, mostly the
call-pair table and the keyed functions on hot type utilities; logs grow by about 70%. The
compiler crates build in about 10 minutes at `-j8` from a warm build directory.

Four suites, through `rustc/coverage-dims-suites.sh` (`--jobs 4`, 2026-10-10, 1 h 30 min):
`tests/incremental` through compiletest (180 tests), every standalone UI test compiled once
(`diag-check`, 18,374), every UI test again under `-Zthreads=8` (`repro-diff --variants threads`,
7,106), and the run-pass tests at `-Copt-level=2` and `-Copt-level=3 -Zmir-opt-level=4`
(`opt-diff --configs O2,O3-mir4`, 3,341). Reports: `~/mirth-work/cov-dims/coverage-report.txt`,
`gaps-arms.md`, and the two copied here, [`coverage-dims.md`](coverage-dims.md) and
[`coverage-config.md`](coverage-config.md).

**Arms.** On the same runs, 70.2% of the blocks in reachable functions ran (panic-only blocks
aside) but only 50.0% of the arms into shared blocks (24,837 of 49,632). Block coverage hides
most in the crates where error recovery joins back into the main path:

| crate | blocks | arms into shared blocks |
|---|---:|---:|
| rustc_hir_typeck | 87.9% | 62.0% (3,648 arms) |
| rustc_hir_analysis | 82.4% | 53.0% (2,744) |
| rustc_trait_selection | 75.9% | 47.3% (3,819) |
| rustc_borrowck | 75.5% | 49.1% (2,400) |
| rustc_middle | 64.7% | 60.4% (3,316) |
| rustc_codegen_ssa | 53.7% | 38.6% (1,427) |
| rustc_target | 23.6% | 4.2% (5,710: per-target ABI code) |

**Configuration arms.** 142 feature gates, 59 options, the edition, 71 target properties and 8
session queries are read by switches in reachable functions. In functions that ran, these arms
were never taken: 117 of the 788 feature-gate arms, 71 of 222 option arms, 22 of 154 edition
arms, 659 of 1,260 target-property arms (every test runs on x86_64 Linux) and 43 of 109 session
arms. Never taken at all, though read at 5 or more places: `feature:coroutine_clone`,
`option:mir_include_spans`, `option:strip`, `target:llvm_abiname` (46), `target:linker_flavor`
(24), `target:cfg_abi`, `target:env`, `target:os`, `target:endian`.

**Keyed engine paths.** 1,124 engine and plumbing functions are keyed by the query. 330 of the
338 dep kinds were seen (the 8 others are not queries: `Null`, `Red`, `SideEffect`,
`AnonZeroDeps`, `TraitSelect`, `CompileCodegenUnit`, `CompileMonoItem`, `Metadata`). Block
coverage counts 2,357 engine blocks as covered; per query, 49,333 (query, block) pairs were
taken. Of the engine's paths: 293 queries ran through `execute_job_incr`, 210 were loaded from
disk or recomputed green (`load_from_disk_or_invoke_provider_green`), 133 were forced from a dep
node, 56 reached `ensure_can_skip_execution`, 166 waited on another thread's job, 28 handled a
cycle, 23 checked a fed value's consistency. The queries with the most engine paths other
queries took and they never did lead the list in [`coverage-dims.md`](coverage-dims.md).

**Incremental transitions.** 25 dependency-graph functions keyed by node kind; 302 kinds seen.
263 kinds went through `try_mark_green`, 159 through `try_force_from_dep_node`, 49 were marked
loaded from disk, 11 had their color read with `node_color`.

**Feature gates consulted.** Of the 268 `Features::<gate>()` accessors, 136 were consulted with
the gate on and off, 9 only while on, 6 only while off (`asm_experimental_reg`,
`cfg_contract_checks`, `cfg_sanitizer_cfi`, `freeze_impls`,
`ref_pat_eat_one_layer_2024_structural`, `rustc_private`: code that checks the gate, reached,
with the gate never on), and 117 never. Features checked through `Features::enabled(sym)` with
a symbol known only at run time are not told apart.

**Type kinds.** 90 functions keyed; 81 ran; every one of the 29 `TyKind`s reached at least one.

**Call pairs.** 162,817 distinct pairs. Of the call graph's 122,984 direct and resolved edges
between functions that both ran, 116,210 were taken (94.5%); 40,006 pairs are not such edges
(function pointers, `dyn`, closures passed through code outside the compiler).

**MIR pass effect.** 73 passes ran. 12 never changed a body's blocks or locals: the checking and
lint passes (which do not change MIR by design), `MentionedItems` (it writes a field outside
what is hashed), and one transforming pass, `SimplifyConstCondition-final`.

**Lock contention.** 37 `Lock::lock` sites were found held under `-Zthreads=8`: the sharded maps
(`sharded.rs`), the diagnostic context (`rustc_errors`), canonical instantiation in
`rustc_infer`, and others listed in [`coverage-dims.md`](coverage-dims.md).

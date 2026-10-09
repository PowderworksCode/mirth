# Which parts of the compiler run

`fixtures/sink` covers Rust's grammar ([`grammar.md`](grammar.md)); this measures how much of
the compiler compiling it reaches. mirth-watch's `[coverage]` mode (`rustc/coverage.toml`)
instruments every function and closure of the compiler's own crates (`rustc_*`) with one call
at entry, which records the function's first call in each process; at exit each rustc
process writes the functions it entered. `rustc/coverage.py` joins those with the site
tables.

    MIRTH_RUST=<rust checkout> BUILD_DIR=<dir> MIRTH_WATCH=rustc/coverage.toml rustc/build.sh
    MIRTH_OUT=<logs> RUSTC=<dir>/<host>/stage1/bin/rustc cargo build   # any number of builds
    rustc/coverage.py --sites <dir>/mirth-sites --logs <logs> [--files] [--unhit rustc_borrowck]

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

`rustc/coverage-flags.py` builds sink once per row of a pairwise transitions table (clean with
the row's A options, then rebuilt after one random edit with its B options). 60 rows, 1,586
rustc processes in all with the run above: **30,657 functions (42.8%)**. The largest gains: MIR
optimization passes (+460, from `-Copt-level` and `-Zmir-opt-level`), `rustc_trait_selection`
(+409, from `-Znext-solver` and edits that fail to compile), `rustc_session` (+351, option
handling), `rustc_thread_pool` (+226, `-Zthreads`).

## rustc's UI tests

`rustc/ui-coverage.py run` compiles each UI test file (the way its `//@` headers say) with the
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

`rustc/ui-fuzz.py` runs the picked tests through incremental rebuilds after the fuzzer's edits,
each compared with a clean build (status, diagnostics, outputs): error reporting and recovery
under incremental compilation, which sink cannot reach.

Running every UI test outside compiletest also showed that compiletest pins the old trait
solver, while nightly defaults to the new one: [`solver.md`](solver.md).

### Through incremental rebuilds

`rustc/ui-fuzz.py` over every UI test that compiles standalone (18,553 files), up to 8 random
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
of the compiler's crates, its outgoing edges, with functions named by `DefPathHash` (the same
from every crate; paths printed through re-exports did not match):

- direct calls; for a call through a trait, the trait item, the closure or function item a
  call through `Fn*` names, and the implementation the caller's types resolve it to, where
  they do;
- functions and closures used as values, in the body and in its promoted constants
  (`&[f, g]`, `&(f as fn())`);
- callees MIR inlining merged into the body (mirth sees MIR after inlining);
- constants' and statics' initializers (`mir_for_ctfe`), for tables of function pointers.

`rustc/callgraph.py` computes reachability from the compiler's `main`s, every function with a
foreign ABI (callbacks from C, C++ and LLVM), every implementation of a trait from outside the
compiler (which `std` may call), and every initializer; a call to a trait item reaches all its
implementations. It over-approximates, so what it leaves out cannot run.

**The check:** every function that coverage saw run must be reachable. The first graph left
11,279 such functions out (re-exported paths); each fix above cut the number, to 0 with
foreign-ABI roots. The proc-macro crates, which run when the compiler is built, are left out.

**Result:** of 71,257 functions, **3,925 (5.5%) cannot run**; 67,332 can. rustdoc is in the
graph too (built with `WITH_RUSTDOC=1`): it runs the compiler's crates as a second entry
point, and without it the rustdoc tests showed 92 functions "unreachable" that ran.

## rustc's own test suites

`rustc/coverage-suites.sh` runs a suite through compiletest (`./x.py test`) on the
instrumented compiler, with `MIRTH_OUT` set, folding logs as they finish
(`rustc/coverage-compact.py`): compiletest handles what a standalone runner cannot (auxiliary
crates, every revision, `minicore` cross-targets, run-make). With `--keep-stage 0 --keep-stage 1`,
and a refusal when the log shows the compiler compiling: a changed mirth runtime once made
`x.py` rebuild the compiler without the instrumentation, so `build.sh` now keeps a runtime per
build directory and replaces it only when it changes.

| suite | tests passed | functions reached |
|---|---:|---:|
| ui | 22,258 | 46,398 |
| ui, without compiletest's `-Znext-solver=coherence` | 20,915 (1,343 fail) | 46,119 |
| run-make | 420 (2 fail) | 33,448 |
| incremental | 180 | 29,842 |
| codegen-llvm | 1,122 | 28,045 |
| crashes | 172 | 27,711 |
| mir-opt | 413 | 26,966 |
| rustdoc-ui | 461 | 25,996 |
| assembly-llvm | 738 | 23,913 |

(The run without the solver pin uses a local compiletest switch, `COMPILETEST_NO_SOLVER_PIN`,
[`hunt/compiletest-solver-pin.patch`](hunt/compiletest-solver-pin.patch).)

**All together, with sink and its option configurations: 48,391 of the 67,332 functions that
can run (71.9%).**

Of the 18,941 that can run and did not:

| what | functions |
|---|---:|
| derived and boilerplate impls (`Debug`, `Clone`, `Hash`, encode/decode, folders) | 6,490 |
| query machinery (vtables, wrappers, cache) | 1,877 |
| diagnostics and errors | 1,410 |
| stable MIR (`rustc_public`): only from tools, `tests/ui-fulldeps` (stage 2) | 1,334 |
| dumps, printing, debug output | 633 |
| other targets, linkers, archives | 324 |
| coverage instrumentation, autodiff, offload (profiler runtime, Enzyme) | 316 |
| the rest, mostly `rustc_middle` (1,106), `rustc_borrowck`, `rustc_mir_transform`, `rustc_hir_typeck`, `rustc_trait_selection` | 6,537 |

The derived impls are reachable only because every implementation of a trait from outside the
compiler is a root; counting a type's impls only when reachable code constructs the type
(rapid type analysis) would take most of them out of the denominator.

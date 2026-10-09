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

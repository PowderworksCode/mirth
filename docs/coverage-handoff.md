# Function coverage of rustc: handoff

The goal: every function of the compiler that can run, run by mirth's corpus (the kitchen sink,
rustc's own test suites, and generated runs), measured at the function level. After that, line
(basic-block) coverage, then the value domains functions see. [coverage.md](coverage.md) has
the results so far and how each piece works; this page is how to run it and what to do next.

**Where it stands (2026-10-09):** 50,762 of the 62,516 functions that can run (81.2%); without
the 911 that run only on a compiler bug, 50,715 of 61,605 (82.3%). 8,741 of 71,257 functions
cannot run at all. Every function that ran is reachable in the call graph (0 misses): keep it so.

## The pieces

| what | where |
|---|---|
| rustc checkout at the pin, with mirth's local patches applied | `~/mirth-work/rust` (`MIRTH_RUST`) |
| coverage-instrumented compiler: each function records its first call | `~/mirth-work/build-cov`, sites in `build-cov/mirth-sites` |
| call-graph compiler: each body writes its edges and demands | `~/mirth-work/build-cg`, graph in `build-cg/mirth-sites/*.graph` |
| folded coverage of each run | `~/mirth-work/cov-suites/<run>/union.txt` |
| sink and option-configuration logs | `~/mirth-work/cov-sink`, `~/mirth-work/cov-flags` |
| UI test lists (all runnable; greedy set cover) | `~/mirth-work/ui-cov/{all-runnable,picked}.json` |
| the report, the gap list | `~/mirth-work/coverage-report.txt`, `gaps.md`, `gaps.json` |

The pin is in `rustc/pins.env` (nightly-2026-10-06, rustc `ea137335b`). The local patches
(mirth's reuse check, `RUSTC_VERIFY_REUSE`; the stopgaps for findings 9–12 and the others;
compiletest's solver-pin switch) are in `docs/hunt/*.patch`; `rustc/regen-patches.sh` lists
them and their order. Both compilers are built from that tree, so their functions agree.

## Running it

All commands from the mirth checkout, with:

```sh
export MIRTH_RUST=~/mirth-work/rust WORK=~/mirth-work
COV_RUSTC=~/mirth-work/build-cov/x86_64-unknown-linux-gnu/stage1/bin/rustc
```

**The report** (a minute):

```sh
rustc/coverage-report.sh                       # totals; writes gaps.md and gaps.json
rustc/coverage-report.sh --why '<function path>'   # how the graph reaches a function
```

`gaps.md` lists what can run and did not, by crate, then file, largest first; `(only panics)`
marks the bug-only ones. `gaps.json` has `reachable_not_hit`, `unreachable`, `ice_only` and
`ran_unreachable` (must stay empty).

**A test suite** through compiletest on the coverage compiler:

```sh
BUILD_DIR=~/mirth-work/build-cov rustc/coverage-suites.sh $WORK/cov-suites <name> tests/<suite> \
  -- --set build.profiler=true
```

It keeps stages 0 and 1 and refuses a result if `x.py` rebuilt the compiler (which would be
uninstrumented). Extra `x test` options go after `--` (`--set build.gdb=/usr/bin/gdb` for
debuginfo). `WRAPPED=1 ... coverage-suites.sh $WORK/cov-suites compiler-unit compiler` runs the
compiler crates' unit tests compiled through mirth-watch; **that run rebuilds the stage 1
sysroot without `std`**, so build `library` again afterwards (below). `rustc/coverage-fulldeps.sh
$WORK` runs the ui-fulldeps tests compiletest skips at stage 1.

**Any other command** (the generators, the fuzzers):

```sh
rustc/coverage-run.sh ui-fuzz python3 rustc/ui-fuzz.py --rustc $COV_RUSTC \
  --tests $MIRTH_RUST/tests/ui --list $WORK/ui-cov/all-runnable.json --work $WORK/ui-fuzz-cov \
  --edits 3 --jobs 6                           # about 45 minutes
rustc/coverage-run.sh generators python3 rustc/coverage-generators.py --rustc $COV_RUSTC \
  --rust $MIRTH_RUST --list $WORK/ui-cov/picked.json --work $WORK/gen-work --jobs 6 \
  --only prints,targets,dumps,links            # prints and targets: minutes; dumps: 30 minutes
```

A new generator is a function in `coverage-generators.py` named in `--only`. A run's directory
under `cov-suites` is picked up by the report automatically; name it with `fulldeps` or
`compiler-unit` if the programs are outside the compiler (their entry points become roots).

**Rebuilding the coverage compiler** (after changing mirth-watch's instrumentation, or when
`std` is missing from its sysroot):

```sh
MIRTH_WATCH=$PWD/rustc/coverage.toml BUILD_DIR=~/mirth-work/build-cov JOBS=10 rustc/build.sh
# std again, with the profiler runtime, without touching the compiler:
cd $MIRTH_RUST && RUSTFLAGS_BOOTSTRAP="-L dependency=$WORK/build-cov/mirth-runtime" \
  RUSTFLAGS_NOT_BOOTSTRAP="-L dependency=$WORK/build-cov/mirth-runtime" \
  RUSTC_WRAPPER_REAL=~/mirth/target/release/mirth-watch \
  MIRTH_RUNTIME=$WORK/build-cov/mirth-runtime/libmirth_runtime.rlib \
  MIRTH_WATCH=~/mirth/rustc/coverage.toml MIRTH_SITES=$WORK/build-cov/mirth-sites \
  ./x.py build --build-dir $WORK/build-cov --stage 1 compiler/rustc library --set build.profiler=true
```

Changing the coverage compiler changes site identities only if function paths change; old
`union.txt` files stay valid otherwise.

**Rebuilding the call graph** (after changing `sites.rs`'s `graph`; about 10 minutes). Cargo does
not know the wrapper changed, so remove what it built first:

```sh
rm -rf $WORK/build-cg/x86_64-unknown-linux-gnu/{stage1-rustc,stage1-tools} $WORK/build-cg/mirth-sites
MIRTH_WATCH=$PWD/rustc/callgraph.toml BUILD_DIR=$WORK/build-cg WITH_RUSTDOC=1 JOBS=8 rustc/build.sh
rustc/coverage-report.sh   # then check: "ran although unreachable: 0"
```

Small experiments with the graph: build one file with mirth-watch directly (a `cg.toml` with
`[scope] crates = ["demo"]` and `[diagnostics] callgraph = true`):

```sh
MIRTH_RUNTIME=$WORK/build-cg/mirth-runtime/libmirth_runtime.rlib MIRTH_WATCH=cg.toml \
  MIRTH_SITES=sites target/release/mirth-watch $(rustup +nightly-2026-10-06 which rustc) \
  --crate-name demo --crate-type lib lib.rs
```

## Things that went wrong, so they do not again

- **Never edit a shell script while it runs**: bash reads it as it goes (a run died at a
  syntax error in the middle of a file being edited). Its compactor and watchdog orphans kept
  holding the build lock afterwards.
- **Do not pipe the instrumented rustc into `head`**: SIGPIPE kills it before the `atexit`
  hook writes its coverage (it looked like errors recorded nothing; they do).
- `pkill -f`/`pgrep -f` with a pattern in the same command matches its own shell: list PIDs,
  then kill by PID.
- `/tmp/sloth-compiler-build.lock` is shared with another project's builds; a run under `flock`
  waits on it.
- The disk is about 94% full (30 GB free): logs are folded and deleted as they finish; do not
  keep raw logs of a large run.
- A finding (a crash or a difference only incremental compilation causes) comes first: pause
  the search, stop-gap it locally (`~/mirth-work/patches`, applied to `~/mirth-work/rust`),
  record it in `docs/hunt.md` with facts, rebuild, resume. Patches stay local; the user writes
  any bug report (the rust repository's LLM policy). ui-fuzz's `P5` results are
  nondeterminism of the clean build itself, not findings.
- ui-fuzz under `RUSTC_VERIFY_REUSE` reports extra diagnostics on the incremental side (the
  check recomputes queries, which emit their lints again): use it for coverage, not for
  diagnostic comparisons, until the check silences recomputation.

## What is left (10,890 functions, the bug-only ones aside)

| what | functions | how to reach it |
|---|---:|---|
| compiler internals by crate (`rustc_middle` 544, `rustc_trait_selection` 414, `rustc_borrowck` 386, `rustc_mir_transform` 338, `rustc_hir_typeck` 317, `rustc_hir_analysis` 300, `rustc_const_eval` 216, ...) | 4,706 | targeted programs (below) |
| query vtables: per-query `try_load_from_disk` (275), `handle_cycle_error` (302), `format_value` (319), `hash_value`, and the providers of 330 queries no run executes | 1,394 | ui-fuzz and the incremental suite under `RUSTC_VERIFY_REUSE` (load and format every cached query); rustdoc and tool-only queries from rustdoc runs; cycle handlers need a cycle per query, many impossible: classify |
| `Debug`/`Display` impls | 1,146 | more dump options over more tests (`-Zunpretty=thir-tree`, `-Zdump-mir`, `-Zverbose-internals` on the full UI list, not the 300 picked); the rest only under `RUSTC_LOG`, which this build compiles out above `info` |
| other derived impls (`Hash`, `PartialEq`, `Ord`, `Clone`, `StableHash`) | 882 | mostly dead in practice: check a sample with `--why`; refine the graph where a demand is spurious |
| codegen, targets, linkers | 810 | more targets × options (`-Ctarget-cpu`, `-Ctarget-feature`, sanitizers, `-Clto`, `-Cembed-bitcode`, `-Zbuild-std`-like cross builds of `core`), linker flavors |
| diagnostics and lints | 694 | programs with the specific error: the error-code docs' examples (`rustc --explain`), lint docs' examples |
| `Encodable`/`Decodable` impls | 586 | the AST's (`rustc_ast::ast`, 297) are never encoded at this pin: check with `--why` and refine (dead by design); the rest, metadata of more item kinds |
| query declarations (`rustc_middle::queries`) | 260 | as the vtables |
| fold/visit impls | 222 | targeted programs |
| `rustc_public` (stable MIR) | 190 | more `ui-fulldeps` programs calling its API |

### The plan

1. **Saturate with generators** (cheap, broad). Next: the dumps over all 18,553 runnable UI
   tests instead of 300; ui-fuzz and `tests/incremental` under `RUSTC_VERIFY_REUSE`; every
   `rustc --explain` example compiled; rustdoc over the UI tests (`rustdoc --document-private-items`,
   `--output-format json`, `--test`); target × option combinations (PICT, as `flag-model.py`).
   Measure each with `coverage-report.sh` and keep the ones that add.
2. **Tighten the denominator** where the graph says reachable but nothing can run it. For each
   suspicious class, `--why` a sample: if the chain has a spurious step (a demand nothing really
   makes), fix the analysis in `sites.rs` (`graph`) or `callgraph.py`, rebuild the graph, and
   check 0 misses. Classes to look at first: the AST's encode/decode, `handle_cycle_error` per
   query, `format_value` (only on a verification failure or `RUSTC_VERIFY_REUSE`), derived
   impls of types only built in tests. A function that only runs on a compiler bug is already
   split out; one that runs only under a debugging option is not dead (a generator reaches it).
3. **Targeted programs for the rest**, file by file from `gaps.md`, largest first: read the
   uncovered functions, write the smallest program (and options) that makes the compiler call
   them, run it with `$COV_RUSTC` and `MIRTH_OUT`, and keep it only if new sites appear in its
   log. Put them in a corpus directory (`fixtures/cover/<crate>/<name>.rs`, `//@` headers as
   compiletest's, run by a `coverage-run.sh` command: not built yet). This is the part that
   scales with agents: one per file or crate, each verifying its own programs against the
   coverage compiler; a workflow over `gaps.md` sections, with a final pass recomputing the
   report.
4. **Report what cannot be reached** with a reason each (needs another host, needs an LLVM
   built with Enzyme or offload, dead by design), so that 100% is "100% of what can run here".
5. **Then lines, then values.** Line coverage: mirth-watch instruments each basic block instead
   of each function's start (`What::Cover` at every block, the site naming the block's span);
   the same pipeline folds them. Value domains: the `[capture]` machinery records argument
   values at function entry; per function, the set (or range) of each argument's values across
   the corpus.

Throughout: when a generator or program finds an incremental bug or a crash, stop and follow the
finding loop above.

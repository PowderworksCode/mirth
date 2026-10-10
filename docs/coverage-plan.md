# Coverage beyond blocks: a plan

Function and block coverage say which code of the compiler ran. They do not say which way a
branch went into a block that other paths also reach, which query a generic engine function
ran for, which kind of type reached a function, which diagnostics and feature combinations were
exercised, or what happened outside the compiler's own crates (the standard library, LLVM).
This plan proposes those dimensions, ranks them by bugs caught per unit of effort, and gives a
build order. M1, M3 and the codegen half of M5 are built (the parts that need no rebuild of the
instrumented compiler); their numbers are in [`coverage.md`](coverage.md#beyond-functions-and-blocks).

## Where coverage stands

| measure | denominator | covered | source |
|---|---|---|---|
| functions | 62,516 reachable by the call graph (of 71,257 instrumented; 8,741 cannot run) | 50,762 (81.2%); 82.3% without the 911 that only panic | [`coverage.md`](coverage.md), all suites |
| basic blocks | about 696,000 in reachable functions, panic-only blocks aside; 100,208 blocks of logging macros counted apart | 80.7% (2026-10-09 block run, all suites) | `mirth-lab callgraph --block-gaps` |
| grammar alternatives | Ur's Rust grammar, per labeled alternative and literal | per fixture | `mirth-lab grammar-coverage` |
| options | accepted `-C`/`-Z` values and pairs | covering arrays, code reached per row | `flag-universe`, `coverage-flags` |
| UI tests | functions each test adds over a baseline | a greedy pick of tests | `ui-coverage` |

The checks of the third batch (rustdoc-diff, lint-check, gate-mutate, abi-diff, ...) are being
measured as suites now (`rustc/coverage-checks.sh`), and coverage-guided gate-mutate is being
built on `mirth/covfuzz`. Both use the site set described next.

### How the current measure works

`mirth-watch` (`crates/mirth-watch/src/sites.rs`, `instrument`) rewrites each body in scope
(`rustc/coverage.toml`: `rustc_*`) before code generation: under `[coverage] functions` it
inserts a call to the runtime's `cover(site)` at entry, under `blocks` at the start of every
other non-cleanup block, tagging blocks every path from which panics (`panic_only_blocks`) and
blocks that are all logging-macro code (`log_only`). A site is a 64-bit identity of
`"{caller}|cover|{block}"`. The runtime (`crates/mirth-runtime/src/coverage.rs`) keeps a
lock-free open-addressed set of 4M slots (32 MiB); a site's first hit inserts it, later hits
cost one atomic load. At exit the set is written to the process's log under `MIRTH_OUT`;
`coverage-compact` folds logs into a suite's `union.txt`; `mirth-lab callgraph` divides by what
the call graph (`rustc/callgraph.toml`, `[diagnostics] callgraph`) says can run.

Two limits matter for everything below:

- **A generic body has one site per block, not per instance.** The query engine
  (`rustc_query_impl::execution::try_execute_query<C, INCR>`), the dependency graph
  (`rustc_middle::dep_graph::graph::try_mark_green`), folders and visitors are each one body
  for hundreds of queries and types. A block in `try_execute_query` counts as covered once any
  query takes it.
- **A block reached from several predecessors hides which predecessor.** The two arms of an
  `if` that join, or a `match` whose arms fall into a shared continuation, are one site.

The other mechanisms mirth already has: frames (`[[frames]]`: enter, exit, captured
arguments through the `Capture` trait, which takes strings, paths and integers, or `Debug`),
calls (`[[calls]]`, before a call to a matching function, with captured operands), statics
(`[statics]`, each touch of mutable or interior-mutable state), the call graph (`edge`, `body`,
`demand`, `specbound` lines), and the patches (`verify-reuse.patch`, `report-untracked.patch`,
which also turns every option field read into a `read_<option>()` call).

## The dimensions, ranked

Ranked by the bugs a gap in the dimension would have hidden (the survey of 1,000 bugs in
[`checks.md`](checks.md): 350 ICEs, 114 wrong rejections, 56 miscompiles, 18 incremental,
13 nondeterminism, 22 diagnostic, 28 lint, 31 link or target; Part 1: 144 ICEs reached through
a nightly feature gate, 63 to 73 through the new solver) against the effort to build it and its
cost per compilation.

| rank | dimension | measures | hook | denominator | effort | runtime cost |
|---:|---|---|---|---|---|---|
| 1 | branch arms | each `SwitchInt` arm, including ones into shared blocks | mirth-watch: a site on each arm whose target has several predecessors | `SwitchInt` arms in reachable functions, minus arms into panic-only or unreachable blocks | 2–3 days | about +30% sites, same per-hit cost |
| 2 | configuration branches | branch arms whose condition comes from a feature gate, an option, the edition or a target property | static tag in mirth-watch over rank 1's arms; no new runtime | the tagged arms | 2 days after rank 1 | none |
| 3 | keyed engine coverage | blocks of the query engine and dependency graph, per query (`QueryVTable::dep_kind`) or node kind | mirth-watch: keyed sites, `site ^ key` from an argument projection | per query: engine blocks its vtable's static flags allow (`eval_always`, `cache_on_disk_local`, `feedable`) | 4–5 days | one field load and a hash per engine block |
| 4 | diagnostics | error codes, lints, diagnostic structs and suggestion applicabilities emitted; delayed-bug sites reached | external: the JSON diagnostics every sweep already keeps; function coverage of each `#[derive(Diagnostic)]` `into_diag` | `rustc_error_codes`' codes, `rustc -W help`'s lints, every `into_diag` impl, every `span_delayed_bug` call site | 2 days | none |
| 5 | feature gates consulted | each feature accessor (`Features::<feature>()`) called while the feature is on, and while off | mirth-watch: capture the accessor's return value at exit (one bit) | the unstable features (`rustc_feature::unstable`), times on/off | 2 days | negligible |
| 6 | incremental transitions | per dep-node kind: new, green from disk, green in memory, red and recomputed, forced, loaded vs recomputed, verified | keyed coverage (rank 3) over `try_mark_green`, `try_mark_previous_green`, `try_load_from_disk`, plus the reuse patch's comparisons | dep-node kinds × transitions their flags allow | 3 days after rank 3 | as rank 3 |
| 7 | dispatch and call pairs | which impl a trait call reached from which call site (k = 1 caller context) | runtime: thread-local current function; at entry, site `hash(caller, callee)` | the call graph's `call`, `resolved` and dispatch edges in reachable code | 4 days | a TLS read and write per call |
| 8 | metadata tables | each rmeta table and lazy kind encoded and decoded, per crate type | external: mirth's metadata records (P3's `written` column) | `define_tables!` (82 tables) × crate types × encode/decode | 1–2 days | none (data exists) |
| 9 | MIR pass effect | each pass that changed a body, per body kind (fn, const, promoted, coroutine) and opt level | rustc-side patch: hash the body before and after each pass in `run_passes_inner`, behind an env var | passes in the pipeline × levels | 2 days | one body hash per pass when on |
| 10 | codegen and target | LLVM intrinsics, calling conventions, target features, linkage kinds emitted; callconv per target | external: the IR and assembly abi-diff, opt-diff and xlink already produce | rustc's lowering arms (`rustc_codegen_llvm::intrinsic`), `rustc_target::callconv` per arch, the target list | 2–3 days | none |
| 11 | type and term kinds | variants of `TyKind`, `ConstKind`, `RegionKind`, `PredicateKind` reaching selected functions | mirth-watch: keyed coverage with the key a discriminant read through the interned pointer | per selected function: variants the function's own matches name, plus "any" | 4 days | a discriminant read per selected function entry |
| 12 | standard library at run time | std functions and blocks the corpus programs execute | mirth-watch over `std`, `core`, `alloc` in the std build | std's reachable functions from the public API | 3 days and a std rebuild | std programs run slower |
| 13 | parallel contention | which locks and shared maps were contended under `-Zthreads` | rustc-side patch: count contended acquisitions per call site in `rustc_data_structures::sync` | lock and shard acquisition sites | 3 days | small, only with threads |
| — | grammar alternatives | exists | Ur | Ur's grammar | done | — |
| — | option pairs tied to code | exists in part | `coverage-flags` | covering arrays | extend | — |
| — | MC/DC conditions, k > 1 paths | not recommended | | | | |

### 1. Branch arms

**Why.** The biggest blind spot in block coverage. Error recovery in rustc often branches into a
shared continuation: `if let Some(x) = ... { report } ` then carry on. A program that only takes
one arm marks the continuation covered. Wrong rejections (114) and ICEs in error recovery are
decided by which arm runs, not by whether the join block runs.

**How.** In `instrument`, for each block whose terminator is `SwitchInt`, for each target
(including `otherwise`) whose block has more than one predecessor (`body.basic_blocks.predecessors()`),
split the edge: a new block with one `cover(site)` call that jumps to the target, site
`"{caller}|arm|{block}|{index}"`. Arms into a target with one predecessor need nothing: the
target's block site already says the arm ran. Add `What::Arm { panics, log }` next to
`What::Block`, reusing `panic_only_blocks` and `log_only` for the target. `mirth::emit::Build`
already inserts calls; it needs a helper to append a block. Cleanup edges stay out, as today.

**Denominator.** Arms of `SwitchInt` in functions the call graph says can run, minus arms into
panic-only blocks, logging-macro blocks, and `unreachable` targets. `callgraph` gains an `arms:`
line beside `blocks:` and an `--arm-gaps` list.

**Cost.** Static counts first (a mirth-watch pass that counts such arms without instrumenting;
about 0.5 extra sites per block is the estimate to check). The runtime set has 4M slots and
uses under 1M today; arms fit. Per hit the cost equals a block's.

### 2. Configuration branches

**Why.** Part 1 of `checks.md`: 144 of 334 crash bugs needed a nightly feature gate, 22 needed
`-Zassumptions-on-binders`, 63–73 the new solver; options account for most incremental bugs
(`untracked-reads.md`). A gate or option that is consulted but only ever seen in one state is
a combination nothing tests.

**How.** In the same pass, mark an arm as configuration-dependent when its `SwitchInt`
discriminant is a local assigned from a call, in the same block or its single predecessor, to a
`Features::<name>` accessor (`rustc_feature::unstable`, `declare_features!`), a
`read_<option>` getter (present in the patched source), `Session::edition`/`Edition::at_least_*`,
or a `TargetOptions` field read. The site table records the tag and the configuration name. No
runtime change.

**Report.** Per feature and option: arms consulted, arms taken in each direction. Feeds
gate-mutate and flag-walk directly: "gate X is consulted at 14 places; 9 have only ever been
seen with X off".

### 3. Keyed engine coverage

**Why.** The query engine and the dependency graph are generic over the cache type, so block
coverage cannot tell `typeck` from `mir_borrowck`. Incremental bugs (18, and mirth's own findings
1–12) live in the combination of a query and an engine path: loaded from disk, cycle, error,
green without loading. `coverage-handoff.md` lists 1,394 per-query vtable functions
(`try_load_from_disk`, `handle_cycle_error`, `format_value`) that are the visible tip of this.

**How.** A `[coverage.keyed]` table in the config: a pattern over function paths and a key
expression over an argument, for example

```toml
[[coverage.keyed]]
pattern = "rustc_query_impl::execution::*"
key = "0.dep_kind"            # argument 0 (the QueryVTable), field dep_kind
[[coverage.keyed]]
pattern = "rustc_middle::dep_graph::graph::*"
key = "dep_node.kind"         # a named argument's field
```

`mirth-watch` resolves the projection on the argument's type (it already resolves argument
types for `captured`), loads it into a fresh local at function entry, casts it to `u64`, and
passes it to a new runtime entry `cover_keyed(site, key)` at each block, which inserts
`site ^ mix(key)`. The site table writes one row per block with the key's meaning; the report
names keys through the dep-kind table (the runtime can log `dep_kind -> name` once per key
from `QueryVTable::name` with a second projection).

**Denominator.** Per query, the engine blocks its vtable's static flags allow: an
`eval_always` query never loads from disk, a query without `cache_on_disk_local` never reaches
`try_load_from_disk`. The flags are constants in the generated vtables; read them from the
`rustc_middle::queries` declarations once and keep a table.

**Cost.** Engine blocks run millions of times; each keyed hit is a field load, a multiply and
the set probe. Expect a few percent on the instrumented compiler. Sites: about 700 queries ×
about 150 engine blocks at most, about 100,000 possible keyed sites; real ones far fewer.

### 4. Diagnostics

**Why.** 22 diagnostic bugs and 28 lint bugs in the survey, and the ICEs where error
recovery goes wrong (rustdoc-diff's finding 38: delayed bugs whose real error comes from a step
rustdoc skips). An error code no test emits is a recovery path nothing checks.

**How.** All external. Every sweep already keeps rustc's JSON diagnostics. A
`mirth-lab diag-coverage` subcommand folds them: error codes (denominator: `rustc_error_codes`,
including the ones marked no longer emitted), lints (denominator: `rustc -W help`, the
allow-by-default ones under lint-check's widening column), suggestion applicabilities per lint.
Function coverage already has a site for each `into_diag` the `Diagnostic` derive generates
(`rustc_macros/src/diagnostics/diagnostic.rs`), so per diagnostic struct is a filtered view of
the existing union. Delayed-bug call sites (`span_delayed_bug`, `delayed_bug`) are blocks; list
them apart, with whether each was reached, and whether any run reached it without an error
being emitted (that would be the ICE).

**Report.** Codes and lints never emitted, by crate; diagnostic structs never built.

### 5. Feature gates consulted

**Why.** As rank 2, from the other side: not "which branches depend on a gate" but "which
gates were ever asked about while on". 144 crash bugs.

**How.** A `[[frames]]` entry over `rustc_feature::unstable::Features::*` with a new `ret`
capture of the boolean return (the exit hook exists; it needs the return place passed as an
argument). Site `"{accessor}|{value}"`. Also record `Features::incomplete` and `internal`.

**Denominator.** Unstable features (262 at the pin) × on/off. A feature consulted only while off
is reachable code no gate-mutate mutant has turned on.

### 6. Incremental transitions

**Why.** mirth's own domain: P6 bugs come from a node taking a transition (green without
loading, loaded but stale) nobody exercised for its kind.

**How.** Keyed coverage (rank 3) over `try_mark_green`, `try_mark_previous_green`,
`try_load_from_disk` and the forcing path, keyed by `DepNode::kind`. The reuse check
(`verify-reuse.patch`) already visits every green value at the end of a session; with `verbose`
it can count per kind what it compared, which gives "verified" per kind.

**Report.** dep-node kinds × transitions, per suite. Feeds `ui-incr` (being built on
`mirth/reuse-sweep`) and the fuzzer: an edit that makes a new kind take a new transition is
worth keeping.

### 7. Dispatch and call pairs

**Why.** Wrong rejections and miscompiles in trait-heavy code depend on which impl a call
reached. The call graph's dispatch edges are rapid type analysis's guess; which ones run is
not measured.

**How.** The runtime keeps a thread-local "current function site" (set at each function's
`cover`, restored on return by a small exit hook, or approximated by leaving it set, as AFL
does). At entry, `cover(hash(caller, callee))` in a second table. k = 1 only.

**Denominator.** Edges in the call graph between reachable bodies (`edge` lines of kind
`call`, `resolved`, dispatch). Too many to cover fully (millions); report it per crate and use
it as fuzzer feedback, not as a percentage to drive to 100.

### 8. Metadata tables

**Why.** Cross-crate bugs: a table a dependent reads but no test has a writer for in that crate
type. Property 17 ("metadata reads hit entries the writer wrote") checks the entries that are
read; this checks which tables are ever written and read at all.

**How.** mirth's metadata records already have the `written` column (P3). Fold them per table
(`define_tables!` in `rustc_metadata/src/rmeta/mod.rs`, 82 tables) × crate type (lib, rlib,
dylib, proc-macro, staticlib) × encoded/decoded, over every suite that runs the metadata
build.

### 9. MIR pass effect

**Why.** 45 optimization-differential bugs and 56 miscompiles. A pass that runs on every body
but changes few of them is tested only by those few.

**How.** A patch in `rustc_mir_transform/src/pass_manager.rs`, `run_passes_inner`: under
`RUSTC_PASS_EFFECT`, hash each body before and after each pass (the stable hash of its basic
blocks), and print `rustc-pass-effect: <pass> <body kind> changed|unchanged` once per pair per
process. Cheap with the env var off.

**Report.** Passes × body kinds × opt levels: which never changed anything in the corpus.

### 10. Codegen and target

**Why.** 31 link or target bugs, 29 platform-specific, the ABI findings 19, 20 and 44–47. Which
intrinsics and calling conventions the corpus emits is visible in outputs.

**How.** External: scan the IR and assembly that opt-diff (`--emit`), abi-diff and xlink produce
for intrinsic calls, `cc` annotations, target features, linkage kinds. Denominators from rustc's
own lowering (`rustc_codegen_llvm/src/intrinsic.rs` arms; their branch arms with rank 1) and
the target list. Per target: whether its `callconv` module's arms were taken (rank 1 again).

### 11. Type and term kinds

**Why.** Many ICEs are a kind reaching a function that does not expect it (`InherentSelf` in
`AliasConst::type_of`, finding 53). Branch arms catch the functions that match on the kind;
this catches the ones that do not match but pass it on.

**How.** Keyed coverage (rank 3) with the key a discriminant: for an argument of type
`Ty<'tcx>`, project through the interned pointer to `TyKind` and read its discriminant
(`Rvalue::Discriminant`). Never through `Debug`: printing a type runs queries and would change
what the compiler does. Selected functions only (layout, ABI, normalization, const evaluation
entry points): a list in the config.

**Denominator.** Per function, the variants it names in its own matches plus one "other";
report variants seen per function, not a global percentage.

### 12. The standard library at run time

**Why.** 62 standard-library bugs; check 7 (std properties) is not built, so for now this is
measurement more than feedback.

**How.** `rustc/build.sh` builds std with the instrumented compiler; add a `std.toml` scope
(`std`, `core`, `alloc`) with `functions` and `blocks`, and run the run-pass tests with
`MIRTH_OUT`. The call graph needs `library/*` bodies too (`callgraph.toml` scope).

### 13. Parallel contention

**Why.** 13 nondeterminism bugs, and the parallel front end's ICEs. Which shared structures
were contended predicts where interleavings differ.

**How.** A patch counting contended acquisitions (a `try_lock` failure before the blocking
`lock`) per `#[track_caller]` site in `rustc_data_structures::sync::lock` and the sharded maps,
under an env var, printed at exit. Run with `-Zthreads=8` over the corpus (repro-diff does).

### Not recommended

**MC/DC condition coverage.** rustc's own `-Zcoverage-options=mcdc` was removed; reconstructing
decisions from MIR is guesswork for short-circuit chains after lowering. Branch arms (rank 1)
give most of the signal.

**Paths with k > 1.** The number of paths explodes; even k = 1 call pairs (rank 7) are only
useful as fuzzer feedback.

## Build order

| milestone | what | depends on | done when | status (2026-10-10) |
|---|---|---|---|---|
| M1 | static counts: arms per function, keyed-site possibilities, delayed-bug sites | none | a report of denominators, before any runtime change | done: `mirth-lab coverage-static` (source-level counts, with block-site estimates of what ran) |
| M2 | branch arms (rank 1) and configuration tags (rank 2) | M1 | `callgraph` prints `arms:`; `--arm-gaps`; a rebuild of `build-blk` as `build-arm`; all suites rerun | not built here: needs the instrumented compiler rebuilt |
| M3 | diagnostics (rank 4) and metadata tables (rank 8) | none (external) | `mirth-lab diag-coverage` and a metadata-table report over the existing sweeps | done: `diag-coverage` (with `--collect` over the UI corpus), `rmeta-coverage` |
| M4 | keyed coverage (rank 3), then incremental transitions (rank 6) and feature gates (rank 5) | M2's rebuild | per-query and per-kind tables; `ui-incr` and the fuzzers run with it | not built here: needs the instrumented compiler rebuilt |
| M5 | MIR pass effect (rank 9) and codegen/target (rank 10) | none | a patch and an external report | codegen/target done: `mirth-lab codegen-coverage` (with `--collect` to LLVM IR); pass effect not built here (a rustc patch and rebuild) |
| M6 | dispatch pairs (rank 7), type kinds (rank 11) | M4 | fuzzer feedback only | not built here: needs runtime instrumentation |
| M7 | std (rank 12), contention (rank 13) | a std rebuild; check 7 | when check 7 or the parallel work needs them | needs a rebuild |

M1 and M3 can start now and in parallel. M2 is the one compiler rebuild that everything after
it shares; fold ranks 1, 2, 3 and 5 into one rebuild if M4 is ready in time.

## Feeding the fuzzers and the checks

Coverage-guided gate-mutate (`mirth/covfuzz`) reads the set of sites a compilation reached and
keeps mutants that reach new ones. Every runtime dimension above lands in the same set (arms,
keyed sites and call pairs are just more site identities), so the fuzzer gains them without
changes. Two additions make the feedback sharper:

- **Weights.** A new arm or a new configuration arm is worth more than a new block of an
  already covered function; a new incremental transition or a new keyed engine path more than
  either for `ui-incr` and `mirth-lab fuzz`. The site table's tags carry the kind, so the
  fuzzer's energy function can weight by kind.
- **Hit counts.** First-hit sets miss "this loop ran 100 times instead of 2". An optional
  `MIRTH_COUNTS=1` mode with a parallel array of saturating 8-bit counters, bucketed as AFL does
  (1, 2, 3, 4–7, 8–15, 16–31, 32–127, 128+), gives the fuzzer that signal; logs then carry
  (site, bucket) pairs. Off for measurement runs.

For the checks: configuration tags (rank 2) and gates consulted (rank 5) tell gate-mutate and
flag-walk what to turn on; diagnostics (rank 4) tell lint-check and diag-check which codes and
lints no test produces, and the error code docs' examples (`rustc --explain`) fill most;
incremental transitions (rank 6) tell `ui-incr` which edits to add; pass effect (rank 9) tells
opt-diff which passes to isolate.

## Reports

| dimension | report | where |
|---|---|---|
| branch arms | `arms:` line, `gaps-arms.md` by crate and file | `mirth-lab callgraph --arm-gaps` (M2); the M1 estimate: `mirth-lab coverage-static` |
| configuration branches | per feature and option: arms consulted, directions taken | `callgraph --config` |
| keyed engine coverage | per query: engine paths taken / allowed | `mirth-lab coverage --keyed` |
| diagnostics | codes, lints, diagnostic structs never emitted; delayed-bug sites | `mirth-lab diag-coverage` |
| feature gates | features consulted on/off | `coverage --keyed` (feature view) |
| incremental transitions | dep kinds × transitions | `coverage --keyed` (dep-graph view) |
| metadata tables | tables × crate types × encode/decode | `mirth-lab rmeta-coverage` |
| MIR pass effect | passes × body kinds × levels | `opt-diff --pass-effect` |
| codegen and target | intrinsics, conventions, features per target | `mirth-lab codegen-coverage` |

## Log-size budgets

A process log today holds the hit set (8 bytes per site in memory; one line per site in the
log) plus the metadata records. Measured suites fold logs as they finish (`coverage-compact`)
and delete them, so the budget is per process and per in-flight batch.

| addition | sites added (M1 counts, 2026-10-10) | per-process log growth | memory |
|---|---|---|---|
| branch arms | the estimate was about +350,000 (0.5 per block). M1 counts 115,951 source-level arms in reachable functions (0.16 per site), 41,265 of all arms implicit. MIR has more arms than source (decision trees for patterns, desugared `?` and loops), and only arms into a block with several predecessors need a site: expect +100,000 to +200,000 sites (+15% to +25%), to be confirmed by M2's static MIR pass | +15% to +25% | fits the 4M-slot set |
| keyed engine coverage | the estimate was at most about 100,000 (700 queries × 150 blocks). M1: 330 queries × 1,018 engine sites = 307,444 possible pairs; a query takes one path per state, so realized pairs should be tens of thousands | a few thousand lines | fits |
| feature gates (`ret` capture) | 136 features with accessor call sites × on/off = 272 sites; the 109 checked only through generic code need the generic gate (`Features::enabled(sym)`) keyed by the symbol | negligible | fits |
| delayed-bug sites | 242 (already block sites; a list, no new instrumentation) | none | none |
| call pairs | millions static; tens of thousands per process | the largest: a second 4M-slot table (32 MiB) | +32 MiB |
| hit counts (fuzzing only) | none | one byte per site | +4 MiB |

Keep the set's first-hit design for measurement; the counts mode is for the fuzzer only. If a
suite's in-flight logs exceed a few GB, compact more often rather than shrinking the record.

# Checks to add: what the last 1,000 rustc bugs ask for

The 1,000 most recent `C-bug` issues on rust-lang/rust (opened 2026-05-08 to 2026-10-09) were
read one by one. For each issue we recorded the general property the bug violates and an
automatic check (an oracle) for that property that would have caught it, plus the inputs needed
to reach it. The per-issue table is [checks/issues.jsonl](checks/issues.jsonl). This page is the
result, grouped into checks and ranked by how many of those bugs each check would have caught.
[testing-model.md](testing-model.md) explains how reach, measure and observe fit together; this
page is the list for **observe**.

## The shape of the 1,000

| kind | issues |
|---|---:|
| internal compiler error | 350 |
| wrong rejection of a valid program | 114 |
| performance (compile time, code quality) | 62 |
| standard library | 62 |
| miscompile | 56 |
| rustdoc | 51 |
| bootstrap and tooling | 48 |
| unsound acceptance (UB in safe code) | 42 |
| link or target | 31 |
| wrong acceptance | 31 |
| platform-specific | 29 |
| lint | 28 |
| other | 35 |
| diagnostic | 22 |
| incremental | 18 |
| nondeterminism | 13 |
| debuginfo | 8 |

912 can be reproduced from a Linux x86_64 host. **mirth would catch 321 of them today, given an
input that reaches the bug.** Almost all of those 321 are crashes. Incremental compilation is
mirth's one differential check, and it accounts for only about 20 of the 1,000. Everything else
needs a check mirth does not have yet.

## Part 1: crashes (334). The check exists; the inputs are the problem.

mirth already notices an ICE. For these 334 bugs, what was missing was an input that triggers
the crash. The issue labels show what those inputs have in common:

| what reaches it | ICEs |
|---|---:|
| a nightly feature gate | 144 |
| — `generic_const_args` (`F-gca_*`) | 62 |
| — `fn_delegation`, `generic_const_exprs`, `type_alias_impl_trait`, `reborrow`, `inherent_associated_types` | 60 |
| the new trait solver (`WG-trait-system-refactor`, `fixed-by-next-solver`; may overlap) | 63–73 |
| `-Zassumptions-on-binders` | 22 |
| a compiler built with **debug assertions** (`requires-debug-assertions`) | 19 |
| rustdoc | 18 |
| real crates (crater, reports from `cargo build` of a published crate) | many; not labeled |

What this asks of the corpus:

1. **Feature-gate mutation.** For each unstable feature, take the UI tests that use it and
   mutate them (ui-fuzz already edits UI tests): combine the feature with others, put it in
   generic, const, async and trait positions. ICEs cluster on new features, and gca alone
   accounts for 62 of the 334.
2. **Run the whole corpus under `-Znext-solver=globally`**, plus a short list of experimental
   `-Z` flags (`-Zassumptions-on-binders`, `-Zpolonius=next`). This is a configuration column,
   not new inputs ([solver.md](solver.md) found compiletest pins the old solver).
3. **A debug-assertions compiler build** (`rust.debug-assertions = true`). It turns rustc's own
   internal checks on: 19 of these ICEs only fire with it, and it adds those code paths to
   coverage's denominator too.
4. **rustdoc over everything rustc compiles** (`--document-private-items`, `--output-format
   json`, `--test`).
5. **Real crates**, a crater-like corpus (see `stable-regression` below).

## Part 2: the checks to add

Ranked by the number of the 1,000 bugs each would have caught (the in-scope count is in
parentheses). The cost column estimates the effort to build the check in mirth: S is under a
day, M a few days, L a project of its own.

| # | check | bugs | property | oracle | cost |
|---:|---|---:|---|---|:-:|
| 1 | **Equivalent rewrites** (metamorphic) | 63 (62) | Rewriting a program into an equivalent one does not change accept/reject, diagnostics or behavior | Apply meaning-preserving rewrites and compare outcomes | M |
| 2 | **Release-to-release** | 63 (62) | Code accepted (and its output, compile time and code size) on release N stays so on N+1 | Same corpus on consecutive nightlies and releases; flag accept→reject, output, time and size changes | M |
| 3 | **Miri differential** | 52 (51) | An accepted safe program has no UB under Miri, and a UB-free program behaves the same compiled at every optimization level | Miri on every accepted safe program; then compiled -O0/-O3 output must equal Miri's | M |
| 4 | **Optimization and pass differential** | 45 (43) | Output does not depend on `-Copt-level`, `-Zmir-opt-level`, any single MIR pass, `-Ctarget-cpu` or LTO mode | Build each runnable program across a matrix of settings and compare stdout and exit status | S |
| 5 | **Cross-target build and link** | 41 (34) | Every documented target builds `core`/`std` and links a minimal program, with no undefined symbols | `-Zbuild-std` plus a real link for every target, then `nm` for undefined symbols | M |
| 6 | **rustdoc consistency** | 41 (39) | rustdoc accepts what rustc accepts; its output matches the compiler (impls, bounds, re-exports, search index) | Run rustdoc next to rustc; check rustdoc JSON against compiler queries; query the search index for every item | M |
| 7 | **Standard library properties** | 38 (29) | Safe std APIs are panic-safe, never re-enter the allocator, and keep their contracts (`Ord`/`Eq`/`Hash`, iterator laws, round trips) | Property tests with fault injection (panicking comparators, a failing allocator, a recording allocator) under Miri or ASan | M |
| 8 | **Spec conformance** | 33 (33) | Accept/reject and attribute and lang-item rules follow the Reference | Generate valid and invalid variants from Reference rules (signatures, attribute positions, coherence) and check the verdict | L |
| 9 | **Solver differential** | 32 (32) | The old and new trait solvers, and NLL and Polonius, agree on accept/reject and diagnostics; a program accepted by only one is UB-free | Compile everything both ways; send one-sided acceptances to Miri | S |
| 10 | **Scaling and budgets** | 31 (27) | Compile time, memory, stack frames and future sizes grow at most linearly with program size; nothing hangs | Size-parameterized generators (fields, nesting depth, call count); fit growth exponents; timeouts | M |
| 11 | **Codegen expectations** | 29 (28) | Equivalent source forms give comparable code; intrinsics lower to their instruction; nothing needless is saved, spilled or kept | Compare code size between equivalent pairs; check that each intrinsic emits its instruction; check prologues | M |
| 12 | **Incremental vs clean** *(have)* | 21 (17) | An incremental rebuild equals a clean build | mirth today; add fault injection into the incremental cache (delete a work product) | S |
| 13 | **Diagnostic invariants** | 20 (19) | No follow-up errors after a stopping error; every error names user-visible items and user files; the same diagnostic appears inside and outside macros | Remove the broken item and compare error sets; check that names in messages occur in the source or std's public paths; scan for absolute paths | S |
| 14 | **ABI vs clang** | 16 (15) | `extern "C"` lowering (attributes, registers, `bool` and small-int ranges, aggregates) matches clang for the same C signature | Generate C signatures, compile both sides, compare the IR attributes, and call across with junk in the high bits | M |
| 15 | **Determinism** | 15 (15) | Output bytes depend only on inputs: not on the thread count, the path, repeated runs, or unrelated files in search paths | Build twice: with `-Zthreads=1` and `-Zthreads=N`, in two directories, with decoy files; compare hashes | S |
| 16 | **Lint oracles** | 13 (13) | A lint fires only when its premise holds; removing what it flags keeps the program compiling | Evaluate each lint's premise as a const assertion; apply the removal and recompile | M |
| 17 | **Feature gates and stability** | 12 (11) | Nothing unstable (types, attributes, intrinsics, marker traits) is reachable from stable code, by any spelling | Enumerate unstable items; reach each through aliases, re-exports, projections and bodyless items without the gate; expect E0658 | S |
| 18 | **Suggestions apply** | 9 (9) | Applying a compiler or lint suggestion parses, removes the error and keeps the meaning; applying fixes twice changes nothing | Collect the machine-applicable suggestions, apply them, recompile, and repeat | S |
| 19 | **Internal validation on** | 5 (5) | MIR validation and debug assertions hold on every program | `-Zvalidate-mir` on the whole corpus; debug-assertions compiler and std (with Part 1, item 3) | S |
| 20 | **Debugger round trip** | 4 (3) | gdb and lldb show the values the source defines, and pretty-printers terminate | Known values at breakpoints, printed under a timeout, compared with the source | M |
| 21 | **Instrumentation round trip** | 4 (3) | `-Cprofile-generate` and `-Cinstrument-coverage` binaries exit cleanly and their profiles merge and export | Instrument, run, then `llvm-profdata merge` and `llvm-cov export`; flag warnings and crashes | S |

Not counted above: 53 issues with no general automatic oracle (visual rustdoc styling, flaky
tests, LLVM upgrade expectations), 24 bootstrap and tooling issues (some do have properties:
help text matches real defaults, killed builds leave no lock files), and the platform-specific
issues outside a Linux host.

## Part 3: each check in more detail

### 1. Equivalent rewrites (63)

Rewriting a program into an equivalent form must not change its outcome. The bugs found each of
these rewrites broken:

- wrap a call in a generic function (#164019: a const where-clause is unchecked through a
  generic caller);
- replace a type with an alias to it (#163501: `missing lifetime specifier` for an alias of
  `Self`);
- add the annotation the compiler inferred (#163563);
- split one crate into two (#163791);
- move an attribute to its canonical position (#163330);
- reorder items or statements, remove dead items, inline a module, and swap `cfg_attr` and
  `cfg_select` forms.

About half of these bugs are wrong rejections. **The oracle needs no specification:** only the
rewrite itself, applied to every UI test. This belongs in ui-fuzz, whose edit machinery already
exists. Start with: wrap in a generic fn, substitute an alias, add the inferred annotation,
split the crate. Expect noise from legitimate differences (inference with an annotation can
change method resolution), so record the error code as well as the verdict.

### 2. Release-to-release (63)

Over half of these are beta crater regressions: `type annotations needed` (#161913), overflow
evaluating a requirement (#161916), `Self` constructors from derives (#163800), and a vtable
slot left empty that crashed at run time (#161441). Others are compile-time and code-size
regressions. The check: the same corpus on nightly N−1 and N, flagging accept→reject flips,
output changes, and growth in time and size beyond a threshold. The corpus matters more than the
check here: these bugs come from **real crates**. rustup toolchains are already on the box
(64 of them), so the cost is the crate corpus plus disk.

### 3. Miri differential (52)

34 of the 52 are unsound acceptances: safe code that compiles and has UB. Examples are
`impl Trait` bounds assumed not live (#163886), match guards that ignore fake borrows (#161578),
and indexing with a diverging index (#161852). The rest are MIR optimizations that Miri sees
break a program (GVN with packed structs #163782, JumpThreading with unions #161898,
ReferencePropagation #163777). Two oracles: **(a) Miri on every accepted safe program**
(any UB report means the compiler accepted something unsound); **(b) Miri with and without each
MIR pass** (`-Zmir-enable-passes=-X`), and compiled output compared with Miri's. The inputs are
programs that run: the run-pass UI tests and generated programs.

### 4. Optimization and pass differential (45)

28 miscompiles. Examples: SsaRangePropagation trusting optional asserts (#163779),
DeadStoreElimination creating overlapping call arguments (#162997), parameter deduction from a
default impl (#163856), and an optimization introducing a dereference on an unreachable path
(#159591). The check: build every runnable program at `-Copt-level` 0, 1, 2, 3, s and z, with
`-Zmir-opt-level` 0 to 4, each MIR pass toggled singly, and fat or thin LTO; compare output.
**It is cheap, because mirth already has the option matrix (`mirth-lab flag-model`) and edit
transitions; what is missing is running the program and comparing its output.** Plain
compiler-fuzzer programs (Csmith-style, rustlantis) feed it well.

### 5. Cross-target build and link (41)

Examples: x86_64-unknown-uefi missing `fmal` (#164018) and `wcslen` (#163614), gnullvm hello
world (#163889), and target cfgs or asm register classes inconsistent with the target
(#163490). The check: for every target, `-Zbuild-std` of `core` (and `std` where it exists),
link a minimal program with the target's real linker (`rust-lld` for most), and check `nm` for
undefined symbols. The existing link generator stops at `-Clinker=true`, which never runs a
real link.

### 6. rustdoc consistency (41)

rustdoc as a second implementation of the front end. Checks:

- rustdoc accepts what rustc accepts;
- each auto-trait impl rustdoc shows has the compiler's bounds (#162274: a probe that requires
  the trait under the documented bounds must compile);
- every re-exported crate and item appears in the output (#162253);
- every item is found in the search index (#162334);
- rustdoc JSON round-trips.

The visual issues stay manual.

### 7. Standard library properties (38)

- Global-allocator reentrancy in `thread_local`, `park` and `Thread::new` (#160930, #160837,
  #160794). Check: install an allocator that records reentry, run the thread APIs under
  injected failures.
- Panic safety of `BTreeMap::split_off` (#158165). Check: comparators that panic at random
  points, then compare counts.
- Signal handling after `process::exit` from a thread (#161018).
- Contract properties: `Ord`, `Eq` and `Hash` consistency, iterator laws, float parse round
  trips.

These checks belong next to std rather than in the compiler, but mirth can run them on its
corpus under Miri.

### 8. Spec conformance (33)

- A lang item with the wrong signature accepted (`panic_handler` as `unsafe fn`, #162967).
- A negative impl overlapping the builtin dyn impl (#162069).
- `#[track_caller]` on a trait declaration not reaching impls (#163406).
- `try_as_dyn` disagreeing with the bound check (#163322, #162134).

The check: generate variants from a rule of the Reference, or an executable spec such as
a-mir-formality or MiniRust, and compare the verdict. This is the most expensive check, and
partly covered by checks 1, 9 and 17.

### 9. Solver differential (32)

24 wrong rejections under the next solver (#162850, #162648), Polonius-only acceptances that are
UB (#160670, #160669), and diagnostic differences between the solvers (#162919). The check:
compile everything with `-Znext-solver=globally` and with the old solver, and with NLL and
`-Zpolonius=next`. Compare the verdicts and error codes, and run one-sided acceptances under
Miri. **The cheapest check here: `ui-mirth-lab solver-diff` already does half of it**
([solver.md](solver.md)).

### 10. Scaling and budgets (31)

- Exponential future size from nested forwarding (#163195).
- Stack slots growing with the call count (#161568, #161506).
- A loop-vectorizer hang at `-O3` (#163626).
- Superlinear compile time for wide serde derives (#162283).
- A rustdoc hang on recursive generics (#160280).

The check: generators parameterized by N (fields, depth, calls, awaits) at several values of N;
fit the growth of time, memory, `size_of_val` and frame size; flag superlinear growth and
timeouts. [testing-model.md](testing-model.md) already names this check.

### 11. Codegen expectations (29)

Mostly missed optimizations:

- equivalent forms with different code size (`match cmp` vs an if chain, #163316; De Morgan,
  #163247);
- intrinsics not lowering to their instruction;
- an interrupt handler saving unused registers (#163231);
- linker-plugin LTO not inlining across the language boundary (#163652).

One miscompile belongs here: wasm SIMD narrowing intrinsics against their scalar definition
(#157456). That is a differential check: run each intrinsic on random inputs and compare with a
scalar reference.

### 12. Incremental vs clean (21, mirth has it)

Two additions:

- **fault injection into the cache** (#162601: a deleted LTO work product panics instead of
  rebuilding);
- **reuse assertions** (#158955: unchanged upstream CGUs re-optimized under ThinLTO; the check
  asserts no codegen on a no-change rebuild).

Diagnostic deduplication differences (#162901) are already in scope for mirth's diagnostic
comparison.

### 13. Diagnostic invariants (20)

- **No follow-up errors** (#163973 const evaluated after a failed check, #161544 spurious E0034).
  Check: remove the broken item and compare the error sets.
- **No internal names** (#161882 `ZeroablePrimitive`). Check: every identifier in a message
  occurs in the source or std's public paths.
- **No absolute toolchain paths** (#160638).
- **Macro transparency** (#160671: an out-of-range literal inside `concat!` gets no error).

All of these run over the UI tests' existing errors.

### 14. ABI vs clang (16)

The P-critical `bool` return on x86_64 (#163911) and AArch64 (#159244), AAPCS64 homogeneous
aggregates (#161382), ppc64 unions of floats (#162011), and `noundef` on sret (#162902). The
check: generate C signatures (scalars, `bool`, small ints, aggregates, unions, HFA/HVA). Compile
the C side with clang and the Rust side with rustc for each target, and compare call lowering
(IR attributes, or assembly). On the host, also call across the boundary with junk in the high
bits. **The ABI generator already compiles signatures for every target; this adds clang and the
comparison.**

### 15. Determinism (15)

Mostly parallel-frontend reproducibility (#163878, #162202, #162203; mirth found #162202), plus
`.rmeta` depending on unrelated files in the search path (#159677). The check: hash outputs
across `-Zthreads=1` and `-Zthreads=N`, two directories, repeated runs, and decoy files in
`-L` paths.

### 16. Lint oracles (13)

A lint must fire only when its premise holds. #163840 (wide-pointer lint on thin pointers):
evaluate the premise as a const assertion. #161339 (a trivial cast that is not trivial): remove
the flagged construct and recompile. #163604 (deprecation ignoring `rust-version`): compare the
replacement's stabilization version with the crate's MSRV.

### 17. Feature gates and stability (12)

Unstable things reachable from stable: `IntoIter<T, A>` nameable without `allocator_api`
(#163273), `rustc_splat` ungated on bodyless functions (#162639), `UnsafeUnpinned` implementable
(#161630), and Enzyme intrinsics callable directly (#161820). The check: enumerate unstable
items from std's stability attributes and the feature list. Reach each without its gate
through aliases, projections, re-exports, bodyless items and impls, and expect E0658. Cheap,
because the enumeration can be generated.

### 18. Suggestions apply (9)

Suggestions that produce invalid code (`pub pub`, #163293; #161693; #161032) or change meaning
(#163310, #163304). The check: on every UI test with a machine-applicable suggestion, apply it
(as `rustfix` does), recompile, expect the error to disappear and nothing new to appear, then
apply again and expect no change.

### 19–21. Internal validation, debuggers, instrumentation (13)

- **`-Zvalidate-mir` and debug assertions** across the corpus. This also catches the 19
  debug-assertion ICEs in Part 1.
- **gdb and lldb printing known values under a timeout** (#163988, a pretty-printer that hangs).
- **Instrumented binaries whose profiles merge and export** (#158217 corrupt counters, #157358
  `llvm-cov export` crashing).

## What to build first

Ordered by bugs caught per unit of effort, and by what mirth already has:

1. **Optimization and pass differential** (45). It extends the option matrix with a run step.
   Two afternoons.
2. **Solver differential** (32, plus up to 73 ICEs). It is `ui-mirth-lab solver-diff` plus Polonius,
   and the next solver as a configuration column for the whole corpus.
3. **Miri differential** (52). It runs Miri over the run-pass tests, then once per MIR pass.
4. **Equivalent rewrites in ui-fuzz** (63). Four rewrites to start with.
5. **ABI vs clang** (16, including a P-critical miscompile). It extends the ABI generator.
6. **A debug-assertions compiler** for coverage, with `-Zvalidate-mir` (19 ICEs; more
   denominator).
7. **Release-to-release** on a real-crate corpus (63). The corpus is the work.

Together these would have caught about 275 of the 666 non-crash bugs (613 once the 53 with no
oracle are set aside). If the corpus work in Part 1 also reaches the crashes, the total is
about 600 of the 1,000.

## Method and caveats

- Fetched with `gh issue list -R rust-lang/rust --label C-bug --state all --limit 1000`
  (newest first), bodies cut to 3,000 characters.
- Eight Claude Haiku agents classified 125 issues each. Most read only the first 600 to 1,500
  characters of each body, so a detail late in a report can be missed. Some batches share one
  wording of the property and oracle per family. A random sample of 25 was checked against the
  titles and all were plausible. Counts are approximate, good for ranking, not to the unit.
- The agents' 351 family names were merged into the checks above by rules (`canon` in the
  table). Some merges are judgment calls; check 1 and check 8 overlap.
- "Would have caught" assumes the input that reaches the bug is in the corpus. For most
  checks, the input side (feature mutation, real crates, runnable programs) is as much work as
  the check.

## Built (2026-10-09/10)

The first seven checks of the build order, as `mirth-lab` subcommands sharing its `uitest` module, run
over the standalone UI tests at the pin (and real crates for release-to-release). Each has
`--recheck`, `--known` and `--pause-on-finding` for the frontier loop.

| check | script | swept | result |
|---|---|---|---|
| optimization and pass differential | `mirth-lab opt-diff` | 3,217 runnable tests × 13 configurations (opt levels, MIR opt levels, LTO, target CPU, Cranelift) | nothing; Cranelift's gaps (tail calls, some linkages and SIMD intrinsics) noted |
| solver differential | `mirth-lab solver-diff` | 17,634 tests × old/new solver × NLL/Polonius | the 26 rejections and 3 crashes of [`solver.md`](solver.md); Polonius agrees with NLL everywhere |
| Miri differential | `mirth-lab miri-diff` | 3,094 runnable tests at MIR opt levels 0, 2, 4 and natively | nothing; tests asserting unspecified behavior (function pointer equality, ZST addresses) listed |
| equivalent rewrites | `mirth-rewrite` + `mirth-lab rewrite-diff` | 18,624 tests × generic-wrap, alias, reorder, unused | findings 25 (generic-wrap) and 28 (reorder) |
| ABI vs clang | `mirth-lab abi-diff` | 21 main targets × 10 seeds × 300 random signatures | findings 19 and 20; #163911 reproduced; i686 MSVC small-struct returns and a PowerPC64 `inreg` float undecided |
| internal checks on | `mirth-lab crash-diff` + a debug-assertions compiler | 18,624 tests with `-Zvalidate-mir` | findings 21–24 (17 tests) |
| release-to-release | `mirth-lab release-diff` | 87 real repositories, nightly-2026-07-18 → 10-06 | findings 26 and 27; `allocative` (unstable features) noted |

Ten new findings (19–28) in [`hunt.md`](hunt.md), none from the checks mirth had before.

### Second batch (2026-10-10)

| check | script | swept | result |
|---|---|---|---|
| suggestions apply (18) | `mirth-lab suggest-diff` | 17,945 tests without `run-rustfix`, 7,385 machine-applicable suggestions applied one at a time | finding 29: 111 lint fixes break builds (six shapes reduced); error-recovery suggestions that leave the error or do not parse noted |
| diagnostic invariants (13) | `mirth-lab diag-check` | 18,374 tests | finding 30: debug output in two diagnostics; spans all in bounds |
| determinism (15) | `mirth-lab repro-diff` | 6,886 tests × repeat, other directory with `--remap-path-prefix`, `-Zthreads=8`, decoy `-L` library | nothing new: only `-Zthreads` differences, all in the known async fn (#162202) and RPIT (#163878) families |
| feature gates (17) | `mirth-lab gate-check` | 143 unstable attributes × 14 positions; 156 unstable library items with resolvable paths × use, renamed use, glob, impl, value, type | every library spelling gated; finding 31 (an ICE after the gate error for `#[rustc_main]` on non-functions); `#[feature]` outside the crate root only warns (intended) |

### Third batch (2026-10-10)

| check | subcommand | what it found |
|---|---|---|
| 20, debugger round trip | `debug-check` | findings 33 (gdb printers on zero-sized elements) and 34 (the `Ref`/`RefMut` printer); 4,000 programs × 3 opt levels, nothing else: no other wrong value, no hang, no gdb crash |

**debug-check.** Each seed generates a program that builds known values: integers of every
width (bounds and random), floats from bit patterns (signed zeros, infinities, NaN payloads,
subnormals, random), chars and strings with escapes and non-ASCII, arrays, slices, Vec,
VecDeque built to wrap around, HashMap/BTreeMap/HashSet/BTreeSet (small, and large enough for
B-tree internal nodes), Option/Result and niche-optimized enums (`Option<NonZero<_>>`,
`Option<&T>`, `Option<Box<T>>`), Box, Rc/Arc with known strong and weak counts, Weak, RefCell
with live `Ref`/`RefMut` guards, Cell, tuples, generated structs, tuple structs, unit structs,
enums with data and fieldless enums with a `repr` and explicit discriminants, unions (every
field read back from the written one's bytes), references, PhantomData, OsString, `Box<str>`,
`Box<[T]>`, `Rc<str>`, `Arc<[T]>`, PathBuf, vectors of thousands of elements, and an Rc cycle.
gdb (with `rust-gdb`'s printers) stops at a breakpoint and prints every local, and `*v` for
boxes and references, one `-ex` each, under a timeout. The output is parsed into a tree and
compared structurally with what the generator built: a wrong value, length, count, borrow flag
or variant, a missing field, a printer exception, a gdb error, a gdb crash or a timeout is a
finding. At `-Copt-level=1` and `2` an optimized-out value is accepted anywhere, a wrong one
never. The Rc cycle prints to gdb's depth limit and terminates.

Expected classes (gdb's own behaviour, labelled in `known()`):

- **gdb 15 cannot read a 128-bit enum discriminant** ("That operation is not available on
  integers of more than 8 bytes"): `Option<i128>`, `Option<NonZero<u128>>`, a `Cell` or `Vec`
  of them. rustc widens an enum's tag to the alignment of its first field, so `Option<i128>`'s
  tag is described (correctly) as a `u128` at offset 0; `Result<u128, u8>` keeps a `u8` tag and
  prints.
- **gdb prints an array of zero-sized elements as an address** (`[Z; 2]` → `0x7fffffffdc05`,
  `ptype` `[Z; 2]`): gdb's generic array printer treats an array whose element size is 0 like a
  pointer to its first element. `Box<[()]>` prints `0x1` the same way.
- **gdb takes a struct ending in a zero-sized field for an unsized one.** A slice of structs
  whose last field is zero-sized (`PhantomData`, `()`, the `alloc: Global` of Rc, Arc, Weak and
  BTreeMap) prints as a single struct, with the length applied to that field (`b4::V {x: 1, m:
  0x5555555acd61}` for `Box<[V]>` with `struct V { x: u8, m: PhantomData<u8> }`); `v.length`
  gives "There is no member named length" while `(&v).length` gives 2; an empty `Box<[Rc<T>]>`
  reads through its dangling pointer (`Cannot access memory at address 0x8`). The same with
  the Rust printers disabled. `&[T]` is unaffected under rust-gdb (its own printer), `Box<[T]>`
  has none. The same struct with the zero-sized field first prints correctly.

- **gdb reads through an optimized-out pointer to an enum.** At `-Copt-level=1`/`2`, `print b`
  for an optimized-out `Box<E>` or `Box<Option<u32>>` gives "Cannot access memory at address
  0x0" (`info address b`: "Symbol "b" is optimized out"); an optimized-out `Box<S>` of a struct
  prints `<optimized out>`. The same with the Rust printers disabled.

The run (seeds 0..4000, `--jobs 4`, gdb 15.1, nightly-2026-10-06's printers), mismatches and
the programs they occur in:

| class | mismatches | programs |
|---|---:|---:|
| finding 34: the Ref/RefMut printer fails on every guard | 1,620 | 380 |
| finding 33: BTreeMap/BTreeSet show `()` for every zero-sized key or value | 1,277 | 91 |
| finding 33: VecDeque, Vec and slice printers on zero-sized elements | 1,212 | 389 |
| expected: gdb 15 cannot read a 128-bit enum discriminant | 1,057 | 332 |
| expected: gdb prints an array of zero-sized elements as an address | 338 | 110 |
| expected: gdb takes a struct ending in a zero-sized field for an unsized one | 294 | 92 |
| expected: gdb reads through an optimized-out pointer to an enum | 36 | 10 |

Also seen, not counted: Rc and Arc of unsized values (`Rc<str>`, `Arc<[T]>`) show `value` as an
address; `&Path` and `PathBuf` have type patterns in `rust_types.py` but no printer (PathBuf
shows its `inner` OsString through that printer); `Box<dyn Trait>` makes gdb warn "(Internal
error: pc … in read in CU, but not in symtab.)" when it prints the vtable. No lldb on this host.

## Running the checks

The checks are subcommands of `mirth-lab` (`crates/mirth-lab`; `mirth-lab --help` lists them):

```sh
cargo build --release -p mirth-lab
R=~/mirth-work/campaign/rustc/bin/rustc T=~/mirth-work/rust/tests/ui
target/release/mirth-lab opt-diff --rustc $R --cranelift "$(rustup +nightly-2026-10-06 which rustc)" --tests $T --work <dir>
target/release/mirth-lab solver-diff --rustc $R --tests $T --work <dir>
target/release/mirth-lab abi-diff --rustc $R --rust ~/mirth-work/rust --work <dir> --seed 3
target/release/mirth-lab release-diff --corpus ~/proofhouse-repos/rust --old nightly-2026-07-18 --new nightly-2026-10-06 --work <dir>
target/release/mirth-lab debug-check --toolchain nightly-2026-10-06 --work <dir> --seeds 0..4000 --jobs 4
```

Sweeps over UI tests share `--tests`, `--work`, `--only`, `--known`, `--jobs`, `--recheck` and
`--pause-on-finding` (exit 3 at the first finding: the frontier loop). Results go to
`<work>/results.jsonl`, findings to `<work>/findings/<test>/`.

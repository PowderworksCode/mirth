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

#### Assembly-level triage (`abi-diff --asm`, 2026-10-10)

The IR comparison flags representation differences that the backend may lower identically, and
on the non-main targets it reported thousands of them. `--asm` settles each one by where the
arguments and the return value actually go. Each generated function stores every parameter to
an extern volatile global and returns a volatile load. Each side is compiled to MIR after
instruction selection by its own backend: rustc with `-Cllvm-args=-stop-after=finalize-isel`,
clang with `-mllvm -stop-after=finalize-isel`. clang is given rustc's FPU, soft-float ABI and
relocation model. The comparison covers the incoming physical registers in argument order (one
name per register-file location), the incoming stack slots read (outside the register home
area), and the registers the return reads.

Validated on the 21 main targets first:
- finding 20 shows a placement difference;
- finding 19 and #163911 keep the same placement (they are extension contracts, which placement
  cannot show, so they stay findings);
- nothing else differs;
- the PowerPC64 `inreg` float (labelled "needs a run") has the same placement in all 40 cases,
  so it is equivalent.

Run with `--all`, seeds 1–3, 300 functions each, 312 targets.
- **Equivalent (same placement):** about 4,100–5,000 parameter-attribute and parameter-type
  differences per seed, 560 calling-convention differences and 120–220 return differences. These
  are the IR noise.
- **Harness artifacts, fixed in the harness:**
  - clang's default armv7r CPU (cortex-r4) has no FPU, so clang disabled the FP registers. It is
    now given `-mfpu` from rustc's features.
  - rustc's soft-float AArch64 targets need clang's `-mabi=aapcs-soft`.
  - clang built non-PIC code where rustc builds PIC. MIPS PIC code receives its address in `$t9`,
    so clang now gets `-fPIC`/`-fno-pic` matching rustc.
- **What remains**, every placement difference classified:

| class | targets (tier) | functions, seeds 1–3 | verdict | evidence |
|---|---|---:|---|---|
| narrow integer arguments lose `signext`/`zeroext` (same placement, extension contract) | mips64, mips64el, mipsisa64r6(el) `-linux-gnuabi64`/`-muslabi64`, mips64-openwrt (3) | 749 attribute differences | **finding 44**, a regression from #163653 | rustc's caller no longer extends (`sll`/`seb` gone, nightly-2026-07-18 vs 2026-10-06) |
| small aggregate (≤ 8 bytes) returned through sret; clang returns it in r3/r4 | powerpc-unknown-{freebsd,netbsd,openbsd,helenos} (3) | 300 | **finding 46** | clang returns in registers for non-Linux ELF PowerPC32 and agrees with rustc on Linux; FreeBSD's system compiler on powerpc is clang |
| homogeneous float aggregate (incl. a union of one float type) not in VFP registers | thumbv7a-{pc,uwp}-windows-msvc (3) | 74 | **finding 45** | rustc's VFP aggregate rules depend on `cfg_abi == EabiHf`; this target has `llvm-floatabi: hard` but no `eabihf` |
| over-aligned aggregate: natural vs declared alignment | thumbv7a-{pc,uwp}-windows-msvc (3) | 38 | part of finding 45 (undecided which is MSVC's) | rustc's ARM code uses `unadjusted_abi_align`; clang uses the declared alignment on Windows, the natural one on Linux |
| `repr(C)` layout: `i64`/`f64` alignment | m68k-unknown-linux-gnu, m68k-unknown-none-elf (3) | 480 | **finding 47** | rustc 4/8, clang 8/8, GCC's documented default 2/2 |
| scalar `__int128` padded to an even slot by rustc, not by clang | the seven mips64 targets (3) | 728 | **clang differs, rustc matches GCC** | the padding is #163653, fixing #161679 to match GCC; clang 21 does not align `__int128` arguments at all |
| union holding a float/double passed in integer registers by rustc, FP registers by clang | sparc64-*, sparcv9-sun-solaris (2/3) | 185 | clang differs (GCC passes unions in integer registers) | GCC `function_arg_union_value`, from source, not run here |
| 16-byte-aligned aggregate: even-slot alignment | sparc64-*, sparcv9-sun-solaris (2/3) | 30 | clang inconsistent (sometimes no alignment, sometimes an extra slot); rustc aligns to an even slot like GCC | GCC `function_arg_slotno`, from source, not run here |
| 16-byte-aligned small aggregate: declared vs natural alignment | aarch64-unknown-none-softfloat, aarch64_be-, aarch64v8r-, aarch64-unknown-linux-pauthtest (2/3) | 26 | clang inconsistent: on aarch64-linux it uses the natural alignment like rustc; under `aapcs-soft` and `pauthtest` the declared one | the same signature on aarch64-unknown-linux-gnu agrees |
| over-aligned aggregate passed by reference (MSVC rule) | i686-unknown-uefi (2) | 293 | expected: rustc applies MSVC's x86 rules on this target (`is_like_msvc`) while its LLVM triple is `windows-gnu` | rustc's lowering equals clang `--target=i686-pc-windows-msvc` in all 300 functions of a seed |
| register-size aggregate with a non-register-size member (e.g. an 8-byte union with a `short[3]`) returned in edx:eax by rustc, sret by clang | 12 i386 targets with register struct return (Windows, Darwin, BSDs) | 12 | undecided: needs GCC or MSVC (the main-target label "i686 msvc small-struct return") | clang's rule recurses into fields; rustc uses the size |
| float or double arguments without SSE | x86_64-unknown-none (2) | 14 | no reference ABI (soft-float x86-64 has no psABI) | clang classifies float pairs as SSE and its backend then splits them over GPRs |
| packed aggregate | hexagon-* (3) | 3 | undecided | one signature shape |
| 16-byte-aligned aggregate in the parameter save area | powerpc64-ibm-aix (3) | 1 | undecided | one signature |
| BPF | bpfel, bpfeb (3) | not compared | no reference: clang's BPF backend rejects stack arguments and large returns | |
| the Rust side does not compile: ICE `unreachable!("Align is given as power of 2 no larger than 16 bytes")` in `callconv/nvptx64.rs` | nvptx64-nvidia-cuda (2) | whole target | **known, rust-lang/rust#163497** (open; its reproducer is `ptx-kernel` parameters). Here a plain `extern "C" fn g() -> A` with `#[repr(C, align(32))] struct A` ICEs on 1.90.0, 1.98.0 and nightly-2026-10-06; 1.80.0 compiled it (sret, align 32) | |

Also: finding 20's float-and-pointer struct appears on the 32-bit RISC-V and LoongArch targets
(50 functions), and finding 19's missing extension on stack arguments on loongarch32. Both
labels now include those targets.

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
| ABI vs clang | `mirth-lab abi-diff` | 21 main targets × 10 seeds × 300 random signatures; `--asm --all`: 312 targets × 3 seeds | findings 19 and 20; #163911 reproduced; i686 MSVC small-struct returns undecided; the PowerPC64 `inreg` float equivalent (same placement); findings 44–47 on non-main targets (assembly-level triage under check 14) |
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

#### rustdoc consistency (6)

`mirth-lab rustdoc-diff` runs rustdoc (nightly-2026-10-06, the same commit as the campaign
rustc) on every standalone UI test, in HTML, HTML with `--document-private-items`, JSON and
JSON with private items, next to `rustc --emit=metadata`. Findings: rustdoc panics (any test);
rustdoc rejects what rustc accepts; the JSON's reachable ids do not resolve (jsondoclint's
rules); a root `pub use` is missing; an auto-trait impl rustdoc shows does not hold under its
bounds, or a negative one does (a probe appended to the test, compiled by rustc, #162274).

| swept | result |
|---|---|
| 18,624 tests: rustc accepts 7,473 (four rustdoc runs each), rejects 10,969 (one run, for panics); 207 skipped (flags rustdoc does not take, or that stop rustc early); 4,562 auto-trait probes in 577 tests | 19 tests with findings, 7 findings (33–39): two rustdoc panics on code rustc accepts (33: `use {{}}`, a regression from nightly-2026-09-26; 36: const binding through a supertrait), rustdoc rejecting accepted code (34, 35, 37), panics on rejected code (38, stable: a too-large static), dangling JSON ids (39). No re-export missing; no auto-trait impl contradicted by its probe |

Expected, and labelled rather than reported: rustdoc's own lints denied by a test; a test's tiny
`recursion_limit` (rustdoc does more trait work); dangling ids for stripped private items in the
public JSON (open upstream: #113674, #119626, #117718, #112852); the `fn_delegation` ICE
(#155728, 11 tests); rustdoc JSON's `unimplemented!()` for unsafe binders (a FIXME, 5 tests).
rustdoc does not read `#![crate_type]` (only `--crate-type`, as Cargo passes it), so the check
passes the attribute's value. 345 probes are inconclusive (a type that cannot be named from the
crate root, an unrenderable bound).
#### lint oracles (16)

| check | script | swept | result |
|---|---|---|---|
| lint oracles (16) | `mirth-lab lint-check` | 18,624 tests; the 8,055 that compile without errors (lints capped to warnings where a test denies them), with 16 allow-by-default lints turned on; 4,244 have a lint warning, 211 lints in all; 30,100 compilations | findings 40–43 (`let_underscore_drop`; lifetime-lint fixes; `dead_code` on needed traits and opaque-type definitions; `trivial_numeric_casts` on literals); known #110332 and #163369 reproduced; `unreachable_pub` and `missing_copy_implementations` edge cases noted in [`hunt/lint-check.md`](hunt/lint-check.md) |

`lint-check` acts on each warning four ways, and reports when anything else changes:
- `#![allow(lint)]`;
- deleting what a premise lint flags (all `dead_code` items at once with their impls, each
  `unreachable_patterns` arm, `unreachable_code` statements up to the block's tail);
- rewriting to what the premise says is equivalent (`trivial_casts` through a coercion site,
  `trivial_numeric_casts` without the cast, `ambiguous_wide_pointer_comparisons` through a
  const assertion that the operand is two words, `impl Copy` for
  `missing_copy_implementations`);
- applying allow-by-default lints' machine-applicable fixes, which suggest-diff never sees,
  alone and then all of a lint's together.

Failures the lint's design or the edit explains are listed per test under `expected` in
`results.jsonl` and not counted: items only exempt dead code uses, re-exports, macro-generated
users, unreachable code that takes part in inference.
#### debugger round trip (20)

| check | subcommand | what it found |
|---|---|---|
| 20, debugger round trip | `debug-check` | findings 48 (gdb printers on zero-sized elements) and 34 (the `Ref`/`RefMut` printer); 4,000 programs × 3 opt levels, nothing else: no other wrong value, no hang, no gdb crash |

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
| finding 49: the Ref/RefMut printer fails on every guard | 1,620 | 380 |
| finding 48: BTreeMap/BTreeSet show `()` for every zero-sized key or value | 1,277 | 91 |
| finding 48: VecDeque, Vec and slice printers on zero-sized elements | 1,212 | 389 |
| expected: gdb 15 cannot read a 128-bit enum discriminant | 1,057 | 332 |
| expected: gdb prints an array of zero-sized elements as an address | 338 | 110 |
| expected: gdb takes a struct ending in a zero-sized field for an unsized one | 294 | 92 |
| expected: gdb reads through an optimized-out pointer to an enum | 36 | 10 |

Also seen, not counted: Rc and Arc of unsized values (`Rc<str>`, `Arc<[T]>`) show `value` as an
address; `&Path` and `PathBuf` have type patterns in `rust_types.py` but no printer (PathBuf
shows its `inner` OsString through that printer); `Box<dyn Trait>` makes gdb warn "(Internal
error: pc … in read in CU, but not in symtab.)" when it prints the vtable. No lldb on this host.
#### feature-gate mutation (crashes, Part 1)

| check | subcommand | swept | result |
|---|---|---|---|
| feature-gate mutation (Part 1, item 1) | `mirth-lab gate-mutate` | 88,174 mutants of the 4,419 standalone UI tests that enable a feature (348 features; the 37 incomplete ones drawn three times as often): 35% splices of two tests with different features, 30% items moved into a generic fn, async fn, closure, anonymous const, module, trait default or inherent impl, 15% an extra incomplete gate, 20% fuzzer edits; each compiled once, under the test's flags (half), `-Znext-solver=globally` (30%) or `-Zassumptions-on-binders` (20%); about 23,000 mutants an hour on 4 cores | 482 ICEs and 16 timeouts; 197 mutants gave a signature their unmutated test does not give, in 19 signatures and 12 families. Known: #153733 (pin_ergonomics, explicit deref), #156099 (transmutability `Assume`), #151310 (blanket `CoerceUnsized`, fixed after the pin), #156410 (`#[const_continue]` to an associated const), #153735 (gca + `generic_const_exprs` "can't type-check body"), and the async-drop "insta-stable" assertion of closed #162756, still reached through `staged_api` (internal) by two other routes. Looks new: findings 50–55 (one stable-reachable, 44; one hang in the default configuration, 45) |

`gate-mutate` keeps each signature's smallest mutant, reduces it (top-level items, brace blocks,
runs of lines; a step may not introduce E0658), checks the reduced file alone (with `--test`
when the source test uses the harness) and searches rust-lang/rust's issues for the message
(`--triage`); the search only proposes candidates, which were read by hand. Signatures are the
panic's location and the first query on the stack, or a delayed bug's message, so one bug can
show as several signatures (finding 50 as six).

## In-compiler invariants (2026-10-10)

[`hunt/check-invariants.patch`](hunt/check-invariants.patch), applied last on the
verify-reuse stack (`rustc/regen-patches.sh` regenerates it), checks invariants from
[`properties.md`](properties.md) inside rustc on every compilation when `RUSTC_CHECK_INVARIANTS`
is set: a violation prints `rustc-invariant: <property>: <details>` and compilation goes on.
`RUSTC_CHECK_INVARIANTS=selftest` also prints which checks ran (and, for #18, which queries'
results are checked and which are not). Built into `~/mirth-work/rustc-verify13`.

| property | where | what upstream had |
|---|---|---|
| 18: query results contain no inference variables | `rustc_query_impl`: each provider's typed result, before it is erased | nothing. The probe dispatches on the value's type (autoref specialization): `TypeVisitable` values, `EarlyBinder`/`&`/`Option`/`Result` around one, canonical query responses, typeck results' node types, borrowck's hidden types, clauses, impl headers, layouts. 99 of the ~200 query kinds a small program runs are checked; the rest return types without types in them, or `Steal`ed bodies |
| 14: a compile that emitted no error has no error types | end of `analysis`, when no error or delayed bug was emitted: typeck results (tainted, node types), `type_of` and `fn_sig` of every local item | nothing |
| 24: symbol names are injective | `assert_symbols_are_distinct`: local mono items against upstream crates' exported symbols | within a session, in every build (fatal `SymbolAlreadyDefined`); across crates, nothing |
| 15: each metadata record is written once | `TableBuilder::set`: an entry set a second time | nothing |
| 9: layout views agree | type lowering: LLVM's ABI size of the lowered type against the layout's size; and rustc's own expensive layout sanity checks, on in release | the sanity checks in debug builds only |
| 1: interned values are well-formed | `debug_assert_args_compatible`, `debug_assert_alias_term_args_compatible`: argument lists against generics, reported instead of a bug | debug builds only |
| 7: spans are valid | metadata span encoding: `lo <= hi`, `lo` inside its file | `debug_assert!` only |

`mirth-lab invariant-sweep` compiles every standalone UI test with the variable set (to a
binary when the test builds, so codegen's checks run; to metadata otherwise), collects the
lines, and counts an ICE that happens only with the variable set as `env-only-ice` (the
enabled debug assertions panic instead of reporting).

| swept | result |
|---|---|
| 18,624 UI tests: 7,408 compile, 11,198 fail as expected, 17 ICE without the checks too, 1 timeout | 315 tests with findings, all property 15: findings 56 (the crate root's module children, encoded twice in every library with a public item) and 57 (coroutine layouts, encoded twice); proc-macro `def_keys` written twice on purpose (6 tests). Nothing for 18, 14, 24, 9, 1, 7 |
| 12,000 `gate-mutate` mutants with the variable set | no ICE signature the earlier run had not seen (the invariant lines themselves are not collected by gate-mutate) |
### Fourth batch (2026-10-10): mirth's own checks over the corpus

The patched compiler (`rustc-verify12`: [`hunt/verify-reuse.patch`](hunt/verify-reuse.patch) and
[`hunt/report-untracked.patch`](hunt/report-untracked.patch)) had only run its own checks in the
fuzzer, the replays and the flag walks. Both hook the dependency graph, so they act only in an
incremental session.

- `--compiler-checks` (diag-check, gate-mutate): each compile is an incremental session with
  `RUSTC_VERIFY_REUSE=all` (every cached query value recomputed at the end of the session and
  compared) and `RUSTC_REPORT_UNTRACKED`. An untracked read is new unless its (what, file) pair
  or its option is in `rustc/untracked-known.tsv` (every pair the fuzzer, replays and flag walks
  reported, and the options whose verdict in [`untracked-reads.md`](untracked-reads.md) holds
  anywhere); a new site of "source text" is a note. Triaged reuse reports are in
  `rustc/reuse-known.txt`.
- `mirth-lab ui-incr`: P6 over the UI corpus. Per test, a clean incremental session, then three
  rebuilds in it (unchanged; a blank line first, which moves every span; an unused fn appended),
  each compared with a clean build of the same source: diagnostics, output bytes (metadata for
  check tests, the program otherwise, with the per-session suffix of object names removed), and
  the program's output when the bytes differ; with the reuse check and the untracked-read report
  on in every session.

| sweep | swept | result |
|---|---|---|
| `ui-incr` | 18,502 tests (7,249 compile, 11,236 fail as expected; 17 ICE or time out in the clean session) × 3 rebuilds, each against a clean build | finding 58; otherwise no incremental session differs from a clean one: no output byte, program output or diagnostic difference |
| `diag-check --compiler-checks` | 18,374 tests, each a clean incremental session recomputing every cached value | no recomputed value differs; no new untracked read |
| `gate-mutate --compiler-checks` | 3,000 mutants | nothing reported by the checks |

What the checks report, all triaged:

- **Allocation identity in const-eval results** (`rustc/reuse-known.txt`, 222 tests): the
  `PostAnalysis` and `Codegen` evaluations of one constant or promoted share an allocation in a
  clean session and get two after the round trip through the cache. Finding 8's mechanism
  (decoding reserves a fresh `AllocId`; `alloc-dedup-on-decode.patch` covers only allocations
  deduplicated when created). No output byte or program output differed in any of these tests;
  not recorded as a finding.
- **The reuse check's own re-emissions** (4 tests): recomputing a green value runs its provider,
  and a provider that emits a lint emits it again, printed without trimmed paths (the check runs
  under `with_no_trimmed_paths`). `ui-incr` counts an extra diagnostic at a clean one's location
  but worded differently as such a re-emission (a note) and an identical extra one as a
  duplicate (a finding). The patch could silence diagnostics while it recomputes.
- **New sites of known untracked reads**: 46 places read source text (75 site and query pairs,
  in 15 queries: `typeck_root`, `dyn_compatibility_violations`, lint passes, `mir_borrowck`,
  `check_match`, `fn_sig`, `type_of`, `impl_trait_header` and others), each a position or a
  wording computed from raw text, the class [`untracked-reads.md`](untracked-reads.md) describes;
  and the options `future_incompat_test` and `ui_testing` at new lint sites.
- **Harness artifacts fixed on the way**: `-Cincremental` passed after a test's flags became the
  value of a trailing `--cap-lints`; two clean incremental builds differ in the per-session
  suffix of object names (78 tests skipped as nondeterministic until it was removed).

release-diff is not included: it builds with official toolchains, which do not have the patches.

## Running the checks

The checks are subcommands of `mirth-lab` (`crates/mirth-lab`; `mirth-lab --help` lists them):

```sh
cargo build --release -p mirth-lab
R=~/mirth-work/campaign/rustc/bin/rustc T=~/mirth-work/rust/tests/ui
target/release/mirth-lab opt-diff --rustc $R --cranelift "$(rustup +nightly-2026-10-06 which rustc)" --tests $T --work <dir>
target/release/mirth-lab solver-diff --rustc $R --tests $T --work <dir>
target/release/mirth-lab rustdoc-diff --toolchain nightly-2026-10-06 --tests $T --work <dir>
target/release/mirth-lab abi-diff --rustc $R --rust ~/mirth-work/rust --work <dir> --seed 3
target/release/mirth-lab lint-check --rustc $R --tests $T --work <dir>
target/release/mirth-lab ui-incr --rustc $R --tests $T --work <dir>              # P6 over the UI corpus
target/release/mirth-lab diag-check --rustc $R --tests $T --compiler-checks --work <dir>
target/release/mirth-lab gate-mutate --rustc $R --rust ~/mirth-work/rust --work <dir> --count 20000 --jobs 4
target/release/mirth-lab gate-mutate --rustc $R --rust ~/mirth-work/rust --work <dir> --triage
target/release/mirth-lab release-diff --corpus ~/proofhouse-repos/rust --old nightly-2026-07-18 --new nightly-2026-10-06 --work <dir>
target/release/mirth-lab debug-check --toolchain nightly-2026-10-06 --work <dir> --seeds 0..4000 --jobs 4
target/release/mirth-lab invariant-sweep --rustc ~/mirth-work/rustc-verify13/bin/rustc --tests $T --work <dir>
```

Sweeps over UI tests share `--tests`, `--work`, `--only`, `--known`, `--jobs`, `--recheck` and
`--pause-on-finding` (exit 3 at the first finding: the frontier loop). Results go to
`<work>/results.jsonl`, findings to `<work>/findings/<test>/`.

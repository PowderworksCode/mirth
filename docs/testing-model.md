# The testing model: reach, measure, observe

What mirth is growing into: a test suite for the Rust compiler in which every behavior the
compiler can have is reached, we can show that it is, and something checks it. Each part scales
on its own axis, so compute can go into whichever dimension needs it (more programs, more
options, longer edit histories, more checks) without redesigning the rest.

## Three legs, and one that keeps them going

A bug is caught only when **an input reaches the faulty code** and **a check observes the
wrong behavior**. Infinite compute without checks catches nothing: the `bool` FFI miscompile
below was compiled countless times by every test suite and noticed by none. So the suite has
three legs, each answering one question.

**1. Reach: the corpus and its settings.** Can we make the compiler do the thing?

- programs: the kitchen sink (dense: every grammar construct, ~30 nightly features, every
  edition), rustc's own test suites, generated programs (UI-test mutations, grammar-driven
  generation), real crates;
- settings: compiler options (pairwise covering arrays over all 206 `-C`/`-Z` options), targets,
  crate graphs, editions;
- history: edit sequences for incremental compilation, threads and scheduling, file-system
  state, resource limits;
- efficiency: each run discharges obligations (a function reached, a feature pair combined, an
  option pair set, an edit sequence applied); the suite is a set cover over them, kept small,
  with dense programs carrying many obligations each.

**2. Measure: static analysis of the compiler.** Have we reached everything there is?

- the denominator: a call graph of the compiler with what can run at all (trait dispatch gated
  on types built and traits demanded; [coverage.md](coverage.md)), checked against what ran;
- the numerator: instrumented compilers recording what each run reached;
- the gap list: what can run and has not, by crate and file, which tells leg 1 what to write
  next;
- the ladder: functions (now), then basic blocks, then the values each function sees, then
  compiler states (which query results were reused or recomputed, which error-taint flags were
  set). "Every state" is the ideal; each rung is a measurable step toward it.

**3. Observe: checks.** Would we notice if the behavior were wrong?

- crashes and internal compiler errors (free);
- differentials, two builds that must agree: incremental against clean (mirth's first check),
  rustc's C ABI against clang's, optimization levels, codegen backends, trait solvers, one
  thread against many, the compiled program against Miri, reproducibility across runs and paths;
- metamorphic relations: edits that must not change the outcome (renaming, reordering items,
  wrapping a call in a generic function must not turn a rejection into acceptance);
- rustc's own invariants as mirth properties, checked inside the running compiler (CTFE never
  evaluates a body whose const checking failed; no follow-up error from a pass that should have
  been skipped);
- a specification for accept/reject and runtime semantics (the Ferrocene language spec,
  a-mir-formality, MiniRust) where one exists;
- performance: scaling curves over size-parameterized programs (superlinear growth is a bug)
  and comparison with earlier versions.

**The fourth leg: the frontier loop.** When a check fires: minimize, stop-gap the bug in a
local compiler, record the facts, and resume, so that a known bug does not drown the run in
repeats of itself ([hunt.md](hunt.md)).

## The legs multiply

The coverage that matters is *reached and observed*, not reached. Leg 2 shows that leg 1 ran the
code; it does not show that leg 3 would notice a fault there. That part is measured by
**mutation testing**: inject faults into rustc (drop a `zeroext` on a return value, skip setting
an error-taint flag), run the corpus with every check, and list the mutants nothing kills. A
surviving mutant is a place where code runs and no check looks: it says which check to write
next, as the gap list says which input to write next.

So each leg has its own gap list and its own way to spend compute:

| leg | gap list | spending more compute |
|---|---|---|
| reach | `gaps.md` (what can run and has not) | more programs, options, targets, edit sequences, threads |
| measure | functions, then blocks, values, states not yet tracked | finer instrumentation, deeper static analysis |
| observe | surviving mutants | more differential partners, more relations, more invariants |

## A check on the model: the last ten bugs

The ten most recent `C-bug` issues on rust-lang/rust (2026-10-06 to 09), against what mirth had
then:

| issue | what | in scope (Linux compiler) | what was missing |
|---|---|---|---|
| #163911 | `extern "C"` returning `bool` leaves the upper bits unmasked on x86_64 (miscompile, P-critical) | yes; reproduces at mirth's pin | **observe**: comparing with clang's ABI for the same C signature, or calling it from C with junk in the high bits. Reach was there: 15 of the 16 x86_64 calling-convention functions had run, and the ABI generator compiles such functions for every target. |
| #164019 | a const where-clause of an impl is not checked when the call goes through a generic function (wrong acceptance) | yes; reproduces | **observe**: the metamorphic relation "wrapping a call in a generic function keeps a rejection a rejection" |
| #163973 | `const_precise_live_drops`: CTFE runs a body whose const check failed (spurious second error) | yes; reproduces | **observe**: the invariant "no CTFE on a body that failed const checking"; **reach**: the feature with a `Drop` type and a swap through `&mut` |
| #163977 | internal compiler error: unnormalized alias in borrowck | yes | **reach**: real crates (derive macros from crates.io); the crash itself is free to observe |
| #164018 | x86_64-unknown-uefi: link fails, undefined `fmal` | Linux host, cross target | **reach**: `-Zbuild-std` and a real link for every target (the link generator links minicore with `-Clinker=true`) |
| #163988 | an lldb pretty-printer spins forever | debugger tooling | **reach**: debuginfo under lldb (only gdb ran); **observe**: a timeout |
| #163997, #163947, #163926, #163923 | Windows sockets, macOS `rust-lld` rpath, `fs::Dir` platform differences | no | out of scope; the rpath one has a cheap Linux analog (run each shipped binary by path, without rustup) |

Five are compiler bugs visible on Linux. For three, the input was near what mirth already
compiles and the code had already run: **the missing piece was the check**. For two, it was a
realistic input. None was an incremental bug, mirth's one check so far. Leg 3 is the thinnest,
and where effort pays most.

## Next, by leg

- **observe**: the clang ABI differential (an extension of the ABI generator); accept/reject-
  preserving rewrites in ui-fuzz, starting with wrapping a call in a generic function; rustc
  invariants as mirth properties; mutation testing to list the checks still missing.
- **reach**: the plan in [coverage-handoff.md](coverage-handoff.md) toward 100% of functions;
  a crater-like corpus of real crates; `-Zbuild-std` with a real link per target; lldb.
- **measure**: basic blocks, then value domains, then compiler states.

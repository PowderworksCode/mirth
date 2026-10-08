# The flag universe

How many rustc configurations are there, which combinations does rustc refuse, and how many
builds cover every pair or triple of option values? Pinned compiler: nightly-2026-10-06
(ea137335b) with mirth's local patches (`rustc-verify5`), x86_64-unknown-linux-gnu.

## Enumeration

`rustc/flag-universe.py` reads `compiler/rustc_session/src/options.rs`:

| | options | enumerable | free-form (left out) |
|---|---|---|---|
| `-C` | 51 | 26 | 25 |
| `-Z` | 235 | 190 | 45 |
| total | 286 | 216 | 70 |

An option's domain is absence plus: `yes`/`no` for a boolean, present for an option without a
value, the backticked values in its parser's description for an enumerated one, `1` and `16`
for a number. Strings, paths, lists, target features, passes and the like are left out,
except 19 options with hand-picked samples (`SAMPLES` in `flag-universe.py`): the first tables
below were made before these were added, and left out `-Copt-level` (its parser takes a
string), so the walks there ran at opt-level 0. With the samples, 235 options are enumerable,
556 values were tried alone and 508 accepted; the pairs were not tried again.

Every value was tried alone on a one-function lib with `--emit=metadata`: 499 values, 452
accepted, 212 options with at least one accepted value. Then every pair of accepted values of
different options, and every value rejected alone against every accepted value of every other
option: 122,682 compilations.

## Which combinations rustc refuses

Among values accepted alone, only **4 pairs** are rejected together, and they are one rule:
`-Cembed-bitcode=no` with any `-Clto` other than `no`/`off`.

The 47 values rejected alone fall into four groups:

- **Not for this target or environment** (left out): `-Zfixed-x18`, `-Zpacked-stack`,
  `-Zreg-struct-return`, `-Zregparm`, `-Zinstrument-mcount=fentry-*`, the hwaddress,
  kernel-address, kernel-hwaddress, memtag and shadow-call-stack sanitizers,
  `-Cinstrument-coverage` (no `profiler_builtins` in our sysroot), out-of-range numbers
  (`-Cdwarf-version=1`, `-Zregparm=16`), removed options (`-Zno-parallel-backend`), and a
  description whose backticked words are not values (`-Zcross-crate-inline-threshold=no`).
- **Target modifiers** that differ from the sysroot's: `-Zretpoline`,
  `-Zretpoline-external-thunk`, `-Zindirect-branch-cs-prefix`, the memory, thread, dataflow
  and safestack sanitizers. Accepted once `-Cunsafe-allow-abi-mismatch` names them, which every
  row below passes.
- **Needs another option** (pair probe and by hand):

  | value | needs |
  |---|---|
  | `-Zsplit-lto-unit=yes` | `-Clto` = yes/on/thin/fat |
  | `-Zvirtual-function-elimination=yes` | `-Clto` = yes/on/fat |
  | `-Zsanitizer=cfi` | `-Clto` = yes/on/fat (thin is not enough) and `-Ccodegen-units=1` |
  | `-Zsanitizer=kcfi` | `-Cpanic=abort` |
  | `-Zsanitizer-cfi-{diag,recover,minimal-runtime}`, `-Zsanitizer-cfi-canonical-jump-tables=no` | `-Zsanitizer=cfi` |
  | `-Zsanitizer-cfi-minimal-runtime=yes` | also `-Zsanitizer-cfi-recover=yes` or `-Zsanitizer-cfi-diag=yes` |
  | `-Zsanitizer-cfi-{generalize-pointers,normalize-integers}` | `-Zsanitizer=cfi` or `kcfi` |
  | `-Zsanitizer-kcfi-arity=yes` | `-Zsanitizer=kcfi` |
  | `-Cforce-frame-pointers=non-leaf`, `-Cpanic=immediate-abort` | `-Zunstable-options` |
  | `-Zdump-dep-graph=yes` | `-Zquery-dep-graph=yes` |

- **Stops compilation early** (left out of walks, and they made the pair probe report false
  "needs"): `-Chelp`, `-Zhelp`, `-Zparse-crate-root-only=yes`, `-Zno-analysis`,
  `-Zlink-only`, `-Zimplicit-sysroot-deps=no` (needs `#![no_std]`).

So the declared constraints are few: 1 pairwise exclusion and 17 implications. They are in
`rustc/flag-model.py`, which writes a [PICT](https://github.com/microsoft/pict) model.

## Covering array sizes

Values per option are absence plus the accepted values. "untracked" are options marked
`[UNTRACKED]` (no effect on the incremental hash), "tracked" the rest.

| subset | options | all combinations | t=2 rows (lower bound) | t=3 rows (lower bound) |
|---|---|---|---|---|
| untracked | 57 | 10^27.4 | 42 (36) | 230 (144) |
| tracked | 149 | 10^74.4 | 152 (126) | 1,509 (1,008) |
| all | 206 | 10^102.0 | 166 (126) | 1,746 (1,134) |

The lower bound is the product of the largest two (three) domains. Without the constraints
the sizes are about the same (all, t=2: 145; tracked, t=3: 1,275); constraints add rows
because the constrained values can only appear in some rows.

Every row of the all t=2 table and the tracked t=3 table was compiled (metadata, and full
codegen to an rlib) on the trivial crate: **0 rejected** out of 1,675, with 99–137 options set
per row. The constraint list is complete for these tables.

### Transitions

An incremental bug needs a change between two sessions. Modeling each option twice (before
and after, 412 parameters for all options) and covering pairs of those covers every
single-option change `x: v → w` and every "x = v before, y = w after":

| subset | parameters | t=2 rows | t=3 rows |
|---|---|---|---|
| untracked | 114 | 54 | 343 |
| all | 412 | 224 | |

Each row is a clean build with A, then a rebuild with B, compared with a clean build with B.

## Walking transitions on sink

`rustc/flag-walk.py` takes a table from `flag-model.py --transitions --cargo` and, per row,
builds `fixtures/sink` clean with the A options, rebuilds with the B options, builds clean
with the B options, and compares (metadata, object code, binaries, diagnostics, the
program's output). A difference is checked against up to 12 more clean builds first, and
reported as P5 (nondeterminism) if clean builds differ among themselves.

A real workspace adds constraints the trivial crate does not show (`CARGO_DROP` and
`CARGO_NEEDS` in `flag-model.py`): `-Clto` is rejected for rlibs and dylibs, Cargo's target
probe fails on values that need another option, there are no sanitizer runtimes here,
`-Cprefer-dynamic` with `-Cpanic=abort` or LTO cannot link, and so on. Single values on sink:
428 of 466 build; no single option, set the same in both sessions, changes a rebuild.

| walk | rows | compared | findings |
|---|---|---|---|
| all options, pairwise transitions | 207 | 190 | 2 rows: P5, finding 10 |
| untracked options, three-way transitions | 345 | 308 | 37 rows: one ICE, finding 11 |

Three new compiler bugs came out of minimizing rows: 10 and 11 from rows that differed or
crashed, 9 from rows that would not link:

- **9**: with `-Cno-prepopulate-passes -Zshare-generics=no -Zthinlto=yes`, a dylib fails to
  link ([facts](hunt/no-prepopulate-link.md)). Not incremental.
- **10**: with `-g -Clto=thin` and incremental compilation, ThinLTO's input is in codegen
  completion order, so clean builds differ from run to run
  ([facts](hunt/thinlto-module-order.md)).
- **11**: a session with `-Zprint-type-sizes` leaves a dependency that makes the next
  session after an edit panic ([facts](hunt/print-type-sizes-trimmed-paths.md)).

Not bugs: `-Csplit-debuginfo=packed|unpacked` objects name `.dwo` files by session (the
walk skips object and binary comparison there); `-Zlint-llvm-ir` aborts on a known LLVM lint
finding ([#59793](https://github.com/rust-lang/rust/issues/59793)).

## Cost

`fixtures/sink` builds clean in about 2 seconds (dev profile), so one transition row (clean
with A, rebuild with B, clean with B) is a few seconds, and the pairwise transitions walk over
all options (224 rows) is minutes. With fuzzer edits on each row, as in the flag battery
(120 edits per configuration), it is about 27,000 rebuilds, a few hours at the fuzzer's rate
of about 2 edits a second. Three-way over all options at roughly 1,600 rows and 120 edits
each is about a day.

## Reproduce

    rustc/flag-universe.py --rustc <rustc> --source <rust checkout> --work <dir> --jobs 10
    rustc/flag-model.py <dir> all model.txt              # or untracked / tracked, --transitions
    pict model.txt /o:2 /r:1 > rows.tsv
    rustc/flag-rows.py <dir> rows.tsv <rustc> --emit=metadata
    rustc/flag-model.py <dir> untracked tr.txt --transitions --cargo
    pict tr.txt /o:3 /r:1 > tr.tsv
    rustc/flag-walk.py --rustc <rustc> --fixture fixtures/sink --flags <dir> --table tr.tsv --work <walk dir>

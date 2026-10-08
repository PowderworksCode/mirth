# More properties on the same corpus

The fixtures, the fuzzer and the ten replayed crates can check more than incremental against
clean. Three properties from [the survey](properties.md), each a flag or two on the same
builds, all on the compiler with the three fixes and the reuse check
(`~/mirth-work/rustc-verify5`). Scripts in `~/mirth-work/props/`.

## MIR validation at every optimization level

`fixtures/sink` and the ten crates at their latest commits, all targets, clean builds with
`-Zvalidate-mir` at `-Zmir-opt-level=0` to `4`, each with `-Copt-level=0` and `3`: 110 builds.
None failed validation or crashed. (`-Zvalidate-mir` is untracked, so these are not
incremental builds: a reused body would not be validated again.)

The fuzzer with `-Zvalidate-mir -Zmir-opt-level=4` on `fixtures/sink` (520 edits) did not
fail validation either, but it found [finding 8](hunt.md): at `-Zmir-opt-level=3` and above,
an incremental rebuild encodes an allocation from MIR inlined from `core` twice. Every
difference of that run that was replayed (four) goes away with `-Zinline-mir=no`.

## The same tests at `-Copt-level=0` and `-Copt-level=3`

Each crate's test suite at both levels, with debug assertions and overflow checks on for both,
so that only optimization differs. A test whose result differs would be a candidate
miscompilation.

| crate | tests | differ |
|---|---:|---:|
| anyhow | 69 | 0 |
| bitflags | 80 | 0 |
| hashbrown | 114 | 0 |
| indexmap | 192 | 0 |
| itertools | 371 | 0 |
| memchr | 167 | 0 |
| regex | 698 | 0 |
| smallvec | 82 | 0 |
| serde | 399 | 0 |
| thiserror | 67 | 0 |

2,239 tests, none differing; `fixtures/sink`'s 60 runtime checks pass at both levels with
identical output. (Doctests fail at both levels alike: Cargo runs the toolchain's `rustdoc`,
which cannot find the crates the instrumented compiler built, `error[E0463]`. So do the
`trybuild` compile-fail suites, which compare diagnostics with recorded text.)

## The old and new trait solvers

Every crate, all targets, compiles with and without `-Znext-solver=globally`, with no errors
either way. Comparing the metadata is not meaningful as done here: the flag changes Cargo's
hashes, so each file differs in its name and crate hash.

The behaviour is compared instead: each crate's tests under the new solver, at
`-Copt-level=0` with debug assertions, against the old solver's results from the run above.
All 2,239 tests give the same result, and the failures that come from the setup (doctests,
compile-fail suites) are the same errors the same number of times under both solvers.

## Threads

With [`hunt/threads-def-order-stopgap.patch`](hunt/threads-def-order-stopgap.patch) working
around #162202, the fuzzer on the unmodified `fixtures/sink` with `-Zthreads=8` compared 644
incremental rebuilds with clean builds and found no difference.

## More flags through the fuzzer

`fixtures/sink` (with `sink_core::extras`), two workers, 60 edits each, every check on, one
run per flag set: about 100 rebuilds compared with clean builds each.

| flags | result |
|---|---|
| `-Copt-level=1`; `=2`; `=3 -Ccodegen-units=1` | nothing |
| `-Cdebuginfo=0`; `-Copt-level=2 -Cdebuginfo=line-tables-only` | nothing |
| `-Zinline-mir=yes` | nothing |
| `-Copt-level=2 -Zinline-mir=yes -Zinline-mir-threshold=200` | no difference in output, but the reuse check's sharing report of finding 8 (19 times): the same lost sharing, here not reaching the metadata |
| `-Zcross-crate-inline-threshold=always`; `-Zshare-generics=no`; `-Cprefer-dynamic` | nothing |
| `-Cpanic=abort -Copt-level=2`; `-Ctarget-cpu=native`; `-Ctarget-feature=+avx2,+bmi2` | nothing |
| `-Cforce-frame-pointers=yes`; `-Coverflow-checks=off -Cdebug-assertions=off` | nothing |
| `-Zmir-opt-level=2`; `-Csymbol-mangling-version=v0` | nothing |
| `-Ccodegen-units=2`; `=64` | nothing (two codegen-unit reports, the known end-of-file pattern) |
| `-Crelocation-model=static` | does not link the fixture's dylib and proc macro: not tested |

So the two flags that found bugs earlier (`-Copt-level=2` before the debuginfo stopgap, and
`-Zmir-opt-level=4`) were the productive ones; with findings 6 and 8 accounted for, the other
common flags find nothing more on this fixture.

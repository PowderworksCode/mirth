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
hashes, so each file differs in its name and crate hash. The tests under the new solver are
compared with the old solver's results below.

## Threads

With [`hunt/threads-def-order-stopgap.patch`](hunt/threads-def-order-stopgap.patch) working
around #162202, the fuzzer on the unmodified `fixtures/sink` with `-Zthreads=8` compared 644
incremental rebuilds with clean builds and found no difference.

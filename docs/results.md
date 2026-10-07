# Seven edits to rustc's metadata code

Each edit below is a small change to `rustc_metadata` of the kind a
contributor might make: plausible and locally reasonable, and each breaks
something. For each one, the instrumented compiler was rebuilt with the
edit, `rustc/check.sh chain` was run, and so were rustc's own tests that
concern metadata. The edits are in `rustc/edits/`, the full output of each
run is in `docs/edits/`, and `rustc/edits.sh` reproduces all of it.

## Result

| # | edit | mirth | rustc's tests |
|---|---|---|---|
| — | none | P1–P7 hold; the list matches | 180 + 532 + 46 pass |
| 1 | encode the `.rmeta` straight into its final path, skipping the temporary file and the rename | **caught**: P1, and the list shows the rename gone | pass |
| 2 | stop recording deprecations | **caught**: the list shows the table no longer written | **caught**: 5 UI tests |
| 3 | let an environment variable override a value encoded in the crate root | **caught**: P4 | pass |
| 4 | group trait impls in a `std` `HashMap` instead of an `FxIndexMap` | **caught**: P4, P5, P6 | pass |
| 5 | decode every item's `def_kind` when a crate is loaded | **caught**: the list shows 175,644 more reads per process | **caught**: 12 tests hang |
| 6 | drop the dependency each extern query records on its crate | **caught**: the list shows no read tracked | **caught**: 9 incremental tests |
| 7 | keep the metadata's temporary directory | **caught**: P7 | pass |

rustc's tests here are all of `tests/incremental` (180), the UI tests in
`tests/ui/{deprecation,crate-loading,rmeta,extern,cross-crate}` (532, 6
ignored), and the 46 tests in `tests/run-make` that concern metadata,
crate loading, incremental compilation or emitted files (`rustc/suites.sh`
lists them). The run-make tests catch none of the edits on their own: they
pass under every edit except 5, where two of them hang like the others.
That includes edits 1 and 7, which change how the `.rmeta` is written and
published; the run-make tests check the files that result, not how they got
there.

mirth catches all seven. rustc's tests catch three; four pass them.

## What each edit looks like

**1. Write in place.** The list for each library shows the encoder's target
change and the rename disappear:

```diff
-  encode-to      target/debug/build/base/#/out/rmeta*/full.rmeta in encoder::encode_metadata
+  encode-to      target/debug/build/base/#/out/libbase-#.rmeta in encoder::encode_metadata
-  remove_file    target/debug/build/base/#/out/libbase-#.rmeta in fs::encode_and_write_metadata
-  rename         target/debug/build/base/#/out/rmeta*/full.rmeta -> target/debug/build/base/#/out/libbase-#.rmeta in fs::encode_and_write_metadata
```

and P1 names it: `encoded straight to …/libbase-#.rmeta`. A reader in
another process can now open a half-written file. Nothing in this build
raced, which is why only a property about the protocol, not the outcome,
catches it.

**2. Drop deprecations.** `base` has one deprecated function. The writer no
longer encodes its entry, and the reader still asks:

```diff
-       1     1 items  record_some_lazy!(self.tables.lookup_deprecation_entry[def_id] <- depr)
-       7     6 items     6 tracked    1 written  base               lookup_deprecation_entry
+       6     6 items     6 tracked               base               lookup_deprecation_entry
```

The dependent gets "not deprecated" without complaint, so its deprecation
warning should go; that was not checked here. rustc's deprecation UI tests,
which check exactly those warnings, catch the edit too.

**3. An environment variable.** P4:

```text
P4  base (lib)  std::env::var(RUSTC_EXTRA_FILENAME) read by encode_crate_root::{closure#33} while encoding, in encoder::encode_metadata
```

Two builds in the same environment give identical bytes, so no comparison
of outputs can see this. It breaks incremental compilation as soon as the
variable changes between builds, because nothing tracks it.

**4. A `std` `HashMap`.** P4 sees the randomly seeded map constructed while
encoding, and P5 and P6 see its consequence: two clean builds give
different `.rmeta` bytes, because the impls come out in a different order.

**5. Eager decoding.** Every compiler process now reads every item's kind
from every dependency as it loads it:

```diff
+  175644  CrateMetadata::def_kind                            in CStore::register_crate
```

In this fixture that is only slow. In rustc's tests, twelve compiles hang
(six incremental, four UI, two run-make), and every one of them loads a
proc macro; the one inspected was waiting on a
futex with no CPU use. The fixture has no proc macro, so mirth saw the cost
and not the hang. The hang was not reproduced without the instrumentation;
the runtime does nothing in those runs, since nothing sets `MIRTH_OUT`.

**6. An untracked extern query.** Each extern query calls
`tcx.ensure_ok().crate_hash(krate)` so that incremental compilation knows the
result depends on that crate. Without it, every read of a dependency is
untracked:

```diff
-      16     1 items     1 tracked               base               adt_def
+      16     1 items     0 tracked               base               adt_def
```

Comparing the output of an incremental rebuild with a clean build (P6) did
not catch this: the fixture's edit did not make a stale result reach `mid`'s
metadata. rustc's cross-crate incremental tests do catch it, because they
assert which results are reused rather than what is produced. mirth catches
it by recording the dependency itself.

**7. Keep the temporary directory.** P7:

```text
P7  left behind: target/debug/build/base/#/out/rmeta*
```

## What the experiment changed in mirth

The first run caught five of the seven. Two misses turned into fixes:

- **Calls inside closures were recorded but never named.** The driver asked
  for every function's MIR before writing the site table, but not every
  closure's, so a closure's sites were added after the table was written.
  Edit 3's read is inside `stat!("final", || …)`. Fixed in `mirth` itself,
  with a test.
- **Comparing outputs is not enough for incremental compilation.** Edit 6
  changes what rustc reuses, not what it produces for this fixture. The list
  now records, for every query a dependent asks of a crate, whether it
  recorded its dependency on that crate. That needs incremental compilation
  on, so the recorded builds are incremental, as Cargo's debug profile is by
  default.

## Limits

- One fixture of three small crates, no proc macro, no build script.
- Only the run-make tests that concern metadata were run, not all 543.
- Each edit was chosen knowing what mirth watches. They are plausible, but
  they are not a sample of real bugs; replaying past metadata and incremental
  regressions from rustc's history would be the stronger test.
- Linux only so far. mirth's own tests run on Linux, macOS and Windows; the
  instrumented compiler has only been built on Linux.

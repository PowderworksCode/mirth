# Five untracked options change results that incremental compilation reuses; with `-Zno-leak-check`, a rebuild accepts a program a clean build rejects

<!-- Draft comment for rust-lang/rust#84232 ("Audit all UNTRACKED options"). -->

`-Zno-leak-check`, `-C extra-filename`, `-Zemit-stack-sizes`, `-Zcodegen-source-order` and
`-Zbuild-sdylib-interface` are marked `[UNTRACKED]` in `compiler/rustc_session/src/options.rs`,
so they're left out of the dependency-tracking hash. Changing one between two incremental
sessions leaves the second session's results as the first's. A clean build with the second
session's options produces something different.

### `-Zno-leak-check`: an incremental rebuild accepts a program a clean build rejects

The leak check is part of type checking and trait selection
(`rustc_infer/src/infer/relate/higher_ranked.rs`), and whether it runs decides whether some
programs compile. `tests/ui/lub-glb/old-lub-glb-hr-noteq2.rs` is one: it passes with
`-Zno-leak-check` and is rejected without it.

```sh
cp tests/ui/lub-glb/old-lub-glb-hr-noteq2.rs lib.rs
rustc --crate-type lib -C incremental=incr  -Zno-leak-check lib.rs   # compiles
rustc --crate-type lib -C incremental=incr                  lib.rs   # compiles: typeck reused
rustc --crate-type lib -C incremental=clean                 lib.rs   # error[E0308]: `match` arms have incompatible types
```

On `nightly-2026-10-06`, and on 1.75.0 and 1.90.0 with `RUSTC_BOOTSTRAP=1`, the second
command succeeds although the program is rejected without the flag. (On 1.60.0 the program
does not compile either way.)

### `-C extra-filename`: reused metadata keeps the old value

The metadata records the crate's own `-C extra-filename`, and a dependent records each
dependency's, as a hint for finding transitive dependencies' files (`locator.rs`). Since
metadata can be reused (#114669, 1.90), a rebuild with a different `-C extra-filename`
publishes metadata that names the old one:

```sh
rustc --crate-type lib --emit=metadata -C incremental=incr -C extra-filename=-aaa --out-dir out lib.rs
rustc --crate-type lib --emit=metadata -C incremental=incr -C extra-filename=-bbb --out-dir out lib.rs
grep -a -c -- -aaa out/liblib-bbb.rmeta   # 1 on 1.90.0 and the nightly, 0 on 1.89.0
```

The lookup falls back to any matching file and checks the crate hash, so the stale hint
costs at most a wrong first guess; Cargo changes `-C metadata` along with
`-C extra-filename`, which is tracked. It is the same class, found the same way.

### `-Zemit-stack-sizes`, `-Zcodegen-source-order`, `-Zbuild-sdylib-interface`

### Reproduction

`lib.rs` can be any library with non-generic code of its own; the one used here is
`fixtures/audit/lib.rs` in mirth.

```sh
rustc --edition 2021 --crate-type lib -C incremental=incr  --out-dir out       lib.rs
rustc --edition 2021 --crate-type lib -C incremental=incr  --out-dir out       lib.rs -Zemit-stack-sizes
rustc --edition 2021 --crate-type lib -C incremental=clean --out-dir clean-out lib.rs -Zemit-stack-sizes
# count .stack_sizes sections in each rlib's objects
for d in out clean-out; do (mkdir $d/x && cd $d/x && ar x ../*.rlib && for f in *.o; do readelf -S $f; done | grep -c stack_sizes); done
```

On `nightly-2026-10-06` the incremental rebuild has 0 `.stack_sizes` sections and the clean
build 276. With `-Zbuild-sdylib-interface`, which compiles an interface without function
bodies, the clean build prints 27 warnings (unused variables in the bodies it skipped) and
writes different metadata and objects. The incremental rebuild prints the first session's 5
warnings and keeps its full build.

A tool that does this for every untracked boolean option, comparing metadata, each rlib
member (object names have their incremental session suffix removed, since the objects are
otherwise identical), diagnostics and files written, is
[`rustc/audit-options.py`](../../rustc/audit-options.py) in mirth:

```text
(control: no option)                     same
-Csave-temps=yes                         128 files only a clean build writes
-Zbuild-sdylib-interface=yes             metadata; object code (44 rlib members differ or exist on one side); diagnostics (5 lines incrementally, 27 clean)
-Zcodegen-source-order=yes               object code (1 rlib members differ or exist on one side)
-Zemit-stack-sizes=yes                   object code (42 rlib members differ or exist on one side)
```

The other 45 boolean untracked options gave the same output incrementally as clean.
(`-Zdump-dep-graph` and `-Zno-parallel-backend` failed to build this crate either way.)
Untracked options that take a value, given one each (`-Ccodegen-units=1`, `=3`,
`-Zmir-include-spans=yes`, `-Zthreads=4`, `-Zterminal-urls=yes`,
`-Zignore-directory-in-diagnostics-source-blocks`, `-Cstrip=symbols`), also gave the same
output incrementally as clean.
`-Csave-temps` writing no temporaries for reused codegen units is probably fine for a
debugging option.

### Suggested fix

Mark `no_leak_check`, `extra_filename`, `emit_stack_sizes`, `codegen_source_order` and
`build_sdylib_interface` `[TRACKED]` (`extra_filename` perhaps `[TRACKED_NO_CRATE_HASH]`).
An audit like the one above could run in CI over every untracked option, so a new option
marked `[UNTRACKED]` that changes reused output is caught when it is added; that is the
long tail this issue's discussion worries about.

### How it was found

[mirth](https://github.com/PowderworksCode/mirth) builds rustc with a check that reports, at
run time, every read of an `[UNTRACKED]` option (and of other state the dependency graph
does not track) inside a computation whose result incremental compilation may reuse
([`report-untracked.patch`](report-untracked.patch)). Leaving out options that only produce
debugging output, a build of its test workspace reports exactly these five. The first
three were found earlier by writing #66955's pattern as a query over rustc's source and
auditing every untracked boolean option differentially, which did not exercise the leak
check.

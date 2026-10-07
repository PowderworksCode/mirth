# `-Zemit-stack-sizes`, `-Zcodegen-source-order` and `-Zbuild-sdylib-interface` are untracked but change output that incremental compilation reuses

<!-- Draft comment for rust-lang/rust#84232 ("Audit all UNTRACKED options"). -->

All three are marked `[UNTRACKED]` in `compiler/rustc_session/src/options.rs`, so they're
left out of the dependency-tracking hash. Adding one between two incremental sessions
leaves the second session's results as the first's, and the option has no effect. A clean
build with the option produces something different.

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
`-Csave-temps` writing no temporaries for reused codegen units is probably fine for a
debugging option.

### Suggested fix

Mark `emit_stack_sizes`, `codegen_source_order` and `build_sdylib_interface` `[TRACKED]`.
An audit like the one above could run in CI over every untracked option, so a new option
marked `[UNTRACKED]` that changes reused output is caught when it is added; that is the
long tail this issue's discussion worries about.

### How it was found

[mirth](https://github.com/PowderworksCode/mirth) wrote the pattern behind #66955
(`--remap-path-prefix` untracked) as a query over rustc's source, reads of options, then
joined it with the options marked `[UNTRACKED]`. The audit is the differential check this
issue's third comment suggests.

# rustdoc builds its session options without rustc's post-parse adjustments

Facts for finding 34. Found by the rustdoc consistency check (`mirth-lab rustdoc-diff`) on two
UI tests rustc accepts: `sanitizer/cfi/fn-ptr-type-mismatch-recover.rs` (rustdoc rejects it) and
`assumptions_on_binders/next-solver-no-overridden.rs` (rustdoc panics).

rustc turns the parsed command line into session options in `build_session_options`
(`compiler/rustc_session/src/config.rs:2697`), which also adjusts options after parsing.
rustdoc parses `-C` and `-Z` itself (`src/librustdoc/config.rs:415-416`, `CodegenOptions::build`
and `UnstableOptions::build`) and fills the session options in `src/librustdoc/core.rs:271` with
`..Options::default()` for the rest. Two of the adjustments it misses show as differences:

## 1. `-Ccodegen-units` is ignored: CFI with LTO is rejected

```rust
fn main() {}
```

With `-Ccodegen-units=1 -Clto -Zsanitizer=cfi -Ctarget-feature=-crt-static`, `rustc
--emit=metadata` succeeds and `rustdoc` fails:

```
error: `-Zsanitizer=cfi` with `-Clto` requires `-Ccodegen-units=1`
error: Compilation failed, aborting rustdoc
```

`Session::codegen_units` (`session.rs:1192`) reads `opts.cli_forced_codegen_units`. rustc sets
it from `-Ccodegen-units` (`config.rs:2783`, `should_override_cgus_and_disable_thinlto`, then
`cli_forced_codegen_units: codegen_units` at `config.rs:3084`). Under rustdoc it stays `None`
although `cg.codegen_units` is `Some(1)`, so `codegen_units()` is the default 16 and the CFI
check in `validate_commandline_args_with_session_available` (`session.rs:1666`) fails. The same
with 1.80.0, 1.90.0, 1.98.0 (`RUSTC_BOOTSTRAP=1`), nightly-2026-10-06 and nightly-2026-10-10.

## 2. `-Zassumptions-on-binders` does not switch on the next solver: ICE

```rust
#![crate_type = "lib"]
```

With `-Zassumptions-on-binders -Znext-solver=no`, rustc warns "-Zassumptions-on-binders
unconditionally enables the next trait solver; `-Znext-solver=no` is ignored" and compiles.
rustdoc panics:

```
thread 'rustc' panicked at compiler/rustc_infer/src/infer/outlives/obligations.rs:280:9:
assertion failed: self.next_trait_solver()
```

`build_session_options` sets `unstable_opts.next_solver = NextSolverConfig::Globally` whenever
`assumptions_on_binders` is on (`config.rs:2733-2751`); rustdoc's `UnstableOptions::build` result
keeps `next_solver` as given, and `destructure_solver_region_constraints` asserts both flags.
Also on nightly-2026-08-27 (then `obligations.rs:255`). Without `-Znext-solver=no` rustdoc
does not panic.

## Scope

`rustdoc` (and `cargo doc` with these flags in `RUSTDOCFLAGS`) with flags whose effect rustc
derives after parsing. Other adjustments in `build_session_options` were not checked one by one.
Low: both flags are unstable, and rustdoc does no codegen.

Not found in the issue tracker (searched 2026-10-10 for rustdoc with
`-Zassumptions-on-binders`, `destructure_solver_region_constraints`, and the CFI message).

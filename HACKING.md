# Hacking on mirth

Every `rustc_private` workaround mirth needs, and why. They are
version-specific: an API that exists on one nightly can be gone on the next.
`rust-toolchain.toml` pins the compiler these were learned against.

## The compiler's source is the reference

`<sysroot>/lib/rustlib/rustc-src/rust/compiler/` ships with the `rustc-dev`
component (not `rustlib/src/rust`, which is `rust-src`, the standard library
only). `rustc_middle/src/mir/syntax.rs` defines every MIR enum. Documentation
and blog posts describe some other nightly.

## Editing with rust-analyzer

The crates that use `rustc_private` say so in
`[package.metadata.rust-analyzer]`. rust-analyzer also needs the compiler's
source, which `rustc-dev` installs; point it there with

```json
"rust-analyzer.rustc.source": "discover"
```

in the editor's settings.

## Being a wrapper Cargo accepts

- **Cargo puts the real compiler first.** A `RUSTC_WRAPPER` is invoked as
  `<wrapper> <rustc> <args…>`, and rustc's parser would read that path as a
  source file. `mirth::run` drops argument 1 when its file stem is `rustc`.
  By shape, not position, so the same binary also works as `RUSTC=`.
- **Cargo probes the compiler first.** `rustc -vV` and `rustc --print …`
  compile nothing; `rustc_driver::compiler_entrypoint` answers them correctly.
  `examples/count-calls/tests/cargo.rs` compares the answer with the real
  compiler's.
- **What Cargo tells a wrapper.** `CARGO_PRIMARY_PACKAGE` marks the packages
  the user asked to build, `CARGO_MANIFEST_PATH` names the crate's manifest,
  and Cargo's probe has neither. `mirth::config` reads them.
- **sccache.** A global `build.rustc-wrapper = sccache` composes with a mirth
  wrapper. sccache's key does not describe what the plugin did to the MIR, so
  a hit would be an artifact that silently is not instrumented. Set
  `RUSTC_WRAPPER` explicitly when running a plugin.

## Starting at all

A `rustc_private` binary links `librustc_driver` and the toolchain's `std`
dynamically, and neither is on the default loader path.
`mirth_build::link_to_the_toolchain` emits an rpath into `<sysroot>/lib` on
Linux and macOS. Windows has no rpath; `<sysroot>/bin` must be on `PATH`
(Cargo arranges it for `cargo run` and `cargo test`).

rustc finds its own sysroot from where `librustc_driver` was loaded. That is
inference about the loader, so `mirth::run` also passes `--sysroot` with the
path baked in at build time.

## Replacing MIR

- **Through `optimized_mir`.** `Config::override_queries` replaces the
  provider. The override calls
  `rustc_interface::DEFAULT_QUERY_PROVIDERS.queries.optimized_mir` for the
  compiler's body, then asks the plugin. Inspecting and rewriting both happen
  there: elsewhere, `tcx.optimized_mir()` *is* the override, and whichever
  body a query returns first is the one codegen gets.
- **The provider is a bare `fn`.** `override_queries` takes a function
  pointer with nowhere for state, so the plugin lives in a global behind a
  `Mutex` (not a thread-local: the query is asked from any thread).
- **Not for const bodies.** rustc asserts that `optimized_mir` is not asked
  for a body evaluated at compile time, and `DefKind::Fn` is not enough to
  rule that out. `mirth::plugin::has_optimized_mir` also checks
  `hir_body_const_context`. Found compiling `core` with `-Zbuild-std`.
- **Closures too.** After analysis, mirth asks for the optimized MIR of every
  function *and closure* before calling the plugin's `finished`. Code
  generation asks for most bodies only later, so a closure left out is
  rewritten after the plugin has reported, and anything it recorded about
  that body is lost.
- **The MIR inliner runs first.** `optimized_mir` hands over a body the
  inliner has already worked on, so a call to a small function such as
  `std::fs::rename` may be gone, replaced by that function's own calls.
  `mirth-watch` passes `-Zinline-mir=no` to the crates it instruments; code
  generation still inlines afterwards.
- **Nothing checks what is put in.** `optimized_mir` runs after borrow
  checking and unsafety checking, so MIR emitted there is never verified. A
  wrong type surfaces as an ICE in codegen, or not at all.

## Injecting a crate the program never named

- `--extern force:name=path` loads a crate nobody refers to; a plain
  `--extern` is dropped as unused. It needs `-Zunstable-options`.
- `-L dependency=` for the crate's own directory and its `deps` directory.
  rustc finds a dependency of a dependency by searching, never through
  `--extern`, so every crate downstream of an instrumented one needs the
  runtime's directory on its search path. Without it the error names the
  crate being loaded, not the runtime: "can't find crate for `rustc_middle`".
- Its functions have no path to resolve. Mark them
  `#[rustc_diagnostic_item = "…"]` and look them up with
  `tcx.get_diagnostic_item`. Statics cannot carry that attribute: take the
  crate number from a function that can, and search
  `tcx.module_children(DefId { krate, index: CRATE_DEF_INDEX })`.
- Cargo runs `rustdoc` directly, not through the wrapper, so doctests of an
  instrumented crate need the same `--extern force:` and `-L` in
  `RUSTDOCFLAGS`.

## Capturing values

`mirth-watch` writes down arguments by passing a reference to a generic hook,
`argument::<T>(&T)`, with `T: Capture`. MIR built in `optimized_mir` is not
type-checked, so the plugin checks `T` itself and only calls the hook for a
type the runtime accepts.

- **Plain data** (structs and tuples of numbers, such as `DefId`) is captured
  number by number, with field projections; nothing of the program's own
  code runs.
- **Pattern types.** rustc's `newtype_index` types store their value as
  `pattern_type!(u32 is 0..=MAX)`. Such a field is transmuted to its base
  integer (same layout) before it is captured.
- **`Debug`** runs the program's code, so it is used only where the
  configuration asks, and only for a type that implements it, checked with
  `type_implements_trait`.

## API changes between nightlies

Moving the pin breaks things in `rustc_private`. What has changed so far:

| older | now |
|---|---|
| `Rvalue::Use(Operand)` | `Rvalue::Use(Operand, WithRetag)` |
| `Rvalue::Len`, `NullaryOp`, `ShallowInitBox` | gone; a slice's length is `UnOp::PtrMetadata` |
| `StatementKind::Retag`, `Deinit` | gone |
| `Operand::{Copy, Move, Constant}` | plus `Operand::RuntimeChecks` |
| `Rvalue::CheckedBinaryOp` | `BinOp::AddWithOverflow` and friends |
| `providers.optimized_mir` | `providers.queries.optimized_mir` |
| struct literals for `Statement`, `BasicBlockData` | `Statement::new`, `BasicBlockData::new_stmts` |
| `Terminator { source_info, kind }` | plus `attributes` (2026-07-18), renamed `loop_hint_attrs` (2026-10-06) |
| `rustc_driver::run_compiler` | `rustc_driver::compiler_entrypoint` (2026-10-06) |
| `type_of(..).instantiate_identity()` returns `Ty` | returns `Unnormalized<Ty>`; call `skip_normalization()` |

`Ty::new_fn_def` takes a `ty::Binder` around the generic arguments.

## Building rustc through a plugin

Bootstrap's rustc shim runs `RUSTC_WRAPPER_REAL` as `<wrapper> <rustc>
<args…>`, so a plugin can build rustc itself. Four things need care:

- **Stage 0 must not have `rustc-dev`.** Bootstrap copies stage 0's
  libraries into the sysroot it compiles the compiler against. Prebuilt
  `rustc_*` crates there are found ahead of the ones being built, and the
  build fails with "found possibly newer version of crate". The plugin
  itself needs `rustc-dev`, so `rustc/setup.sh` gives bootstrap a copy of
  the pinned nightly with that component's files removed.
- **Crates compiled without the plugin need the runtime too.** Bootstrap
  compiles some crates without `RUSTC_WRAPPER_REAL`, `rustdoc` among them,
  and they link compiler crates that were instrumented. `rustc/build.sh`
  copies the runtime into the stage 0 sysroot, which every compile
  searches.
- **`rustdoc` does not build with the pinned nightly's Cargo.** Its
  per-crate build directories hide the compiler crates `rustdoc` links
  through `rustc_private`, so `tests/run-make`, which needs `rustdoc`, is
  not run. Bootstrap normally uses beta Cargo, which lays out builds the
  old way.
- **Cargo does not know about the wrapper.** Changing the plugin or its
  configuration does not rebuild anything. `rustc/build.sh --again` deletes
  the fingerprints of the crates in scope.

## The allocator

`rustc` uses jemalloc. A plugin binary uses the system allocator unless it
does the same, which measured about 16% slower on a large build. mirth does
not do this yet. `#[global_allocator]` aborts, because `librustc_driver`
already has an allocator compiled in. What works is rustc's own trick: link
`tikv-jemalloc-sys` with `unprefixed_malloc_on_supported_platforms`, and
keep its C symbols with `#[used]` statics so they interpose `malloc` for the
whole process.

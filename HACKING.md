# Hacking on mirth

Every `rustc_private` workaround mirth needs, and why. They are
version-specific: an API that exists on one nightly can be gone on the next.
`rust-toolchain.toml` pins the compiler these were learned against.

## The compiler's source is the reference

`<sysroot>/lib/rustlib/rustc-src/rust/compiler/` ships with the `rustc-dev`
component (not `rustlib/src/rust`, which is `rust-src`, the standard library
only). `rustc_middle/src/mir/syntax.rs` defines every MIR enum. Documentation
and blog posts describe some other nightly.

## Being a wrapper Cargo accepts

- **Cargo puts the real compiler first.** A `RUSTC_WRAPPER` is invoked as
  `<wrapper> <rustc> <args…>`, and rustc's parser would read that path as a
  source file. `mirth::run` drops argument 1 when its file stem is `rustc`.
  By shape, not position, so the same binary also works as `RUSTC=`.
- **Cargo probes the compiler first.** `rustc -vV` and `rustc --print …`
  compile nothing; `rustc_driver::run_compiler` answers them correctly.
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
  rule that out. `mirth::plugin::is_a_function` also checks
  `hir_body_const_context`. Found compiling `core` with `-Zbuild-std`.
- **Nothing checks what is put in.** `optimized_mir` runs after borrow
  checking and unsafety checking, so MIR emitted there is never verified. A
  wrong type surfaces as an ICE in codegen, or not at all.

## Injecting a crate the program never named

- `--extern force:name=path` loads a crate nobody refers to; a plain
  `--extern` is dropped as unused. It needs `-Zunstable-options`.
- `-L dependency=<its deps directory>` beside it, or rustc resolves the
  injected crate's own dependencies against the sysroot's copies and rejects
  them as different crates.
- Its functions have no path to resolve. Mark them
  `#[rustc_diagnostic_item = "…"]` and look them up with
  `tcx.get_diagnostic_item`. Statics cannot carry that attribute: take the
  crate number from a function that can, and search
  `tcx.module_children(DefId { krate, index: CRATE_DEF_INDEX })`.
- Cargo runs `rustdoc` directly, not through the wrapper, so doctests of an
  instrumented crate need the same `--extern force:` and `-L` in
  `RUSTDOCFLAGS`.

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

## The allocator

`rustc` uses jemalloc; a plugin binary uses the system allocator unless it
does the same, which cost about 16% on a large build. `#[global_allocator]`
aborts, because `librustc_driver` already has an allocator compiled in. What
works is rustc's own trick: link `tikv-jemalloc-sys` with
`unprefixed_malloc_on_supported_platforms`, and keep its C symbols with
`#[used]` statics so they interpose `malloc` for the whole process.

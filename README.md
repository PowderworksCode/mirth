# mirth

**Testing properties of a Rust program by rewriting its MIR.** mirth is a
rustc driver that Cargo accepts as a wrapper. It hands a plugin each
function's MIR as the compiler produced it, and the plugin can hand back a
changed body: one that records what the function did, or that stops at a
chosen point.

The first use is the Rust compiler itself. A compiler built through mirth
records what each `rustc` process in a `cargo build` does with crate metadata
(`.rmeta`): what it writes, how it publishes the file, what dependents read,
and whether anything it reads is invisible to incremental compilation. Those
records are blessed as plain lists, and properties are checked across every
process in the build. [docs/plan.md](docs/plan.md) has the plan.

## Status

Early. The plugin library, an example plugin, and `mirth-watch` with its
runtime work on the pinned nightly, tested through Cargo on Linux, macOS and
Windows. The instrumented compiler is next.

## Watching a program

```sh
cargo install --path crates/mirth-watch    # installs cargo-mirth and mirth-watch
cargo mirth run -- program-arguments       # in a project with a mirth.toml
```

`cargo mirth` takes any Cargo command. It reads `mirth.toml` (the format is
documented in `crates/mirth-watch/src/config.rs`; `fixtures/effects/watch.toml`
is an example), compiles the runtime with the plugin's toolchain, and builds
into `target/mirth/target`, apart from the ordinary build. Site tables go to
`target/mirth/sites` and the logs of `run` and `test` to `target/mirth/logs`.

### By hand

```sh
rustc --edition 2024 --crate-type rlib --crate-name mirth_runtime -O \
  crates/mirth-runtime/src/lib.rs --out-dir target/runtime
cargo build -p mirth-watch
MIRTH_WATCH=watch.toml MIRTH_RUNTIME=target/runtime/libmirth_runtime.rlib \
MIRTH_SITES=sites RUSTC_WRAPPER=target/debug/mirth-watch cargo build
MIRTH_OUT=logs ./target/debug/my-program
```

The runtime is compiled with `rustc` directly: Cargo would put its metadata
in a separate `.rmeta`, and the runtime is injected as one file.
`watch.toml` names what to record (`crates/mirth-watch/src/config.rs`
documents it; `fixtures/effects/watch.toml` is an example). `sites/` gets a
table of every instrumented site, written while compiling. `logs/` gets one
log per process, written while it runs. `MIRTH_CRASH=<site>:<n>` aborts the
program on the `n`th arrival at a site marked `point = true`.

## Using it

A plugin is a binary:

```rust
fn main() -> ! {
    mirth::run(env!("MIRTH_SYSROOT"), MyPlugin)
}
```

with a `build.rs` of one line, `mirth_build::link_to_the_toolchain()`. Then:

```sh
RUSTC_WRAPPER=path/to/my-plugin cargo build
```

`examples/count-calls` is the smallest complete plugin.

## Layout

| path | what |
|---|---|
| `crates/mirth` | the plugin library: driver, `optimized_mir` override, crate injection, MIR emission |
| `crates/mirth-build` | what a plugin's `build.rs` has to do |
| `crates/mirth-watch` | a plugin that records the frames, calls, arguments and static touches a configuration names, and can stop a process at a chosen call |
| `crates/mirth-runtime` | what instrumented code calls: one log per process, with timestamps comparable across processes |
| `examples/count-calls` | the smallest plugin, and its tests through Cargo |
| `fixtures/` | small Cargo projects the tests build |
| `docs/` | the plan, and later the properties and results |

`HACKING.md` records every `rustc_private` workaround.

## Platforms

Linux, macOS and Windows. CI builds and tests all three.

## License

Like Rust itself, under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.

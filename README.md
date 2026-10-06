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

Early. The plugin library and an example plugin work on the pinned nightly.

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
| `examples/count-calls` | the smallest plugin, and its tests through Cargo |
| `fixtures/` | small Cargo projects the tests build |
| `docs/` | the plan, and later the properties and results |

`HACKING.md` records every `rustc_private` workaround.

## Platforms

Linux, macOS and Windows. CI builds and tests all three.

## License

Like Rust itself, under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.

# mirth

**Testing properties of a Rust program by rewriting its MIR.** mirth is a
rustc driver that Cargo accepts as a wrapper. It hands a plugin each
function's MIR as the compiler produced it, and the plugin can hand back a
changed body: one that records what the function did, or that stops at a
chosen point.

Its first use is the Rust compiler itself. A compiler built through mirth
records what each `rustc` process in a `cargo build` does with crate metadata
(`.rmeta`): what it writes, how it publishes the file, what dependents read,
and whether those reads are tracked for incremental compilation. The
records are blessed as plain lists, and seven properties are checked across
every process in the build.

## Results

Seven plausible edits to rustc's metadata code were each built into the
instrumented compiler and checked. mirth catches all seven; rustc's own
metadata-related tests catch three.

- [`docs/report.md`](docs/report.md): the experiment, for readers new to it
- [`docs/results.md`](docs/results.md): each edit and the output that caught it
- [`docs/plan.md`](docs/plan.md): the plan the work followed, with the properties

## An instrumented compiler

On Linux, with about 30 GB free:

```sh
export MIRTH_RUST=$HOME/mirth-rust   # where the rustc checkout goes
rustc/setup.sh                        # fetch the pinned commit, configure bootstrap
rustc/build.sh                        # build stage 1 through mirth-watch, then its std
rustc/check.sh chain                  # check fixtures/chain
rustc/edits.sh chain                  # apply, check and revert each edit in rustc/edits
```

The build takes about an hour on 16 cores. `rustc/rmeta.toml` says what is
recorded: the tables each crate writes, which dependency's metadata each
extern query reads and whether it tracks that dependency, crate loading,
every file operation on the way to a published `.rmeta`, and environment,
clock and randomness reads. After changing the plugin or the configuration,
`rustc/build.sh --again` recompiles the crates in scope.

`rustc/check.sh` builds a fixture with Cargo and the instrumented compiler,
then:

- compares what each compiler process did with metadata against the blessed
  list in `tests/rmeta/<fixture>.txt`; `--bless` accepts a changed list;
- runs `mirth check` for P1, P2, P4 and P7;
- builds again and compares every published `.rmeta` byte for byte (P5);
- applies `fixtures/<fixture>/edit`, rebuilds incrementally, and compares
  with a clean build of the edited source (P6).

## Watching any program

```sh
cargo install --path crates/mirth-watch    # installs cargo-mirth and mirth-watch
cargo mirth run -- program-arguments       # in a project with a mirth.toml
```

`cargo mirth` takes any Cargo command. It reads `mirth.toml` (documented in
`crates/mirth-watch/src/config.rs`; `fixtures/effects/watch.toml` is an
example), compiles the runtime with the plugin's toolchain, and builds into
`target/mirth/target`, apart from the ordinary build. Site tables go to
`target/mirth/sites`, and the logs of `run` and `test` to `target/mirth/logs`.

The same by hand:

```sh
rustc --edition 2024 --crate-type rlib --crate-name mirth_runtime -O \
  crates/mirth-runtime/src/lib.rs --out-dir target/runtime
cargo build -p mirth-watch
MIRTH_WATCH=watch.toml MIRTH_RUNTIME=target/runtime/libmirth_runtime.rlib \
MIRTH_SITES=sites RUSTC_WRAPPER=target/debug/mirth-watch cargo build
MIRTH_OUT=logs ./target/debug/my-program
```

The runtime is compiled with `rustc` directly because Cargo would put its
metadata in a separate `.rmeta`, and it is injected as one file. `sites/`
gets a table of every instrumented site, written while compiling; `logs/`
gets one log per process, written while it runs. `MIRTH_CRASH=<site>:<n>`
aborts the program on the `n`th arrival at a site marked `point = true`.

## Writing a plugin

A plugin is a binary:

```rust
fn main() -> ! {
    mirth::run(env!("MIRTH_SYSROOT"), MyPlugin)
}
```

with a `build.rs` of one line, `mirth_build::link_to_the_toolchain()`, run
as `RUSTC_WRAPPER=path/to/my-plugin cargo build`. `examples/count-calls` is
the smallest complete one. `HACKING.md` records every `rustc_private`
workaround mirth needed.

## Layout

| Path | What |
|---|---|
| `crates/mirth` | the plugin library: driver, `optimized_mir` override, crate injection, MIR building |
| `crates/mirth-build` | what a plugin's `build.rs` has to do |
| `crates/mirth-watch` | the plugin that records frames, calls, arguments and static touches, and `cargo mirth` |
| `crates/mirth-runtime` | what instrumented code calls: one log per process, with timestamps comparable across processes |
| `crates/mirth-cli` | `mirth record`, `report` and `check` |
| `rustc/` | building the instrumented compiler, checking fixtures with it, and the edits |
| `fixtures/` | small Cargo projects the tests and checks build |
| `tests/rmeta/` | blessed lists, one per fixture |
| `examples/count-calls` | the smallest plugin, tested through Cargo |
| `docs/` | the report, the results of each edit, and the plan |

## Platforms

mirth, its runtime and `cargo mirth` are tested on Linux, macOS and Windows
in CI. The instrumented compiler has only been built on Linux.

## License

Like Rust itself, under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.

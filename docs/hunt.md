# Looking for new bugs

The edits and the replayed regressions test whether mirth catches bugs that
someone put in. This tests whether it finds bugs nobody knew were there: the
unmodified pinned compiler (`ea137335b`), a wider fixture, and more
incremental edits.

`fixtures/wide` has a proc-macro crate (a derive and an attribute macro), a
library with a build script that generates code, built-in derives, const
generics, a generic associated type, `impl Trait` and `async fn` in traits,
and an exported `macro_rules!`, used by a second library and a binary.
`fixtures/wide/edits` has ten changes: a private body, a new enum variant,
a doc comment, spans only, two items swapped, the derive's output, the
build script's output, a const generic argument, a macro's expansion, and
a new private function.

`rustc/hunt.sh <fixture>` builds the fixture eight times with
`-Zthreads=8` and compares the `.rmeta` files (P5). Then, for each edit, it
compares an incremental rebuild after the edit with a clean build of the
edited source (P6), with `-Zincremental-verify-ich`, single-threaded and
with `-Zthreads=8`. `rustc/check.sh wide` runs the ordinary checks.

## Findings

| | what | status |
|---|---|---|
| 1 | single-threaded incremental rebuilds encode `Generics::param_def_id_to_index` in a different order from clean builds | **looks new**; reduced to 2 lines; a one-line change fixes it |
| 2 | single-threaded incremental rebuilds of a crate using a proc-macro derive encode hygiene data that clean builds do not | **looks new**; reduced, not root-caused |
| 3 | with `-Zthreads=8`, two traits with `-> impl Trait` methods give different metadata from run to run | known: [#162202](https://github.com/rust-lang/rust/issues/162202) |

All three reproduce with the official `nightly-2026-10-06`, without mirth:
`docs/hunt/repro.sh` runs them. None of them was searched for; P5 and P6
reported them on the first run of the new fixture.

Everything else held: P1, P2, P4 and P7 on the recorded clean build; the
touch-only rebuild reused every crate's metadata; and
`-Zincremental-verify-ich` found no unstable fingerprint in 20 incremental
rebuilds.

### 1. `param_def_id_to_index` order after an incremental rebuild

```rust
pub struct Grid<A, B, C>(A, B, C);
impl<A, B, C> Grid<A, B, C> { pub const AREA: usize = 1; }
```

Build this with `-C incremental`, add a comment line at the top, build
again, and compare the `.rmeta` with a clean build of the same source: they
differ. Two clean builds agree, and so do an incremental rebuild with no
change and a clean build.

Apart from the 16-byte hash in the header, only a few bytes differ, and they
are the same `(DefIndex, u32)` pairs in a different order:
`Generics::param_def_id_to_index`, an `FxHashMap<DefId, u32>` that
`TyEncodable` writes in iteration order. In the incremental session the
`generics_of` result is loaded from the incremental cache. The likely
mechanism, not verified: decoding builds the map by inserting in the
encoded order, and with colliding hashes that gives a different layout,
and so a different order, from the map rustc built in the first place. That fits
what the reduction showed: whether it reproduces depends on unrelated items
in the crate, which change the `DefId`s, and so the hashes.

The same field is the cause given in
[#163878](https://github.com/rust-lang/rust/issues/163878), which reports
it under `-Zthreads`. This is the same field with no threads at all, in an
ordinary `cargo build` after an edit. It matters more than it might look:
since [#154724](https://github.com/rust-lang/rust/pull/154724) the crate
hash is computed from the metadata bytes, so an incremental build and a
clean build of the same source get different crate hashes, and every
dependent is rebuilt.

`docs/hunt/generics-index-map.patch` changes the field to an `FxIndexMap`,
which keeps insertion order through encoding and decoding. With it, the
reduced case and nine of the ten edits give identical bytes. The
single-threaded edit that still fails is finding 2.

### 2. Hygiene data after an incremental rebuild

With finding 1 fixed, adding a variant to an enum in `wide_core` and the
matching arm in `wide_user` still gives `wide_user` different metadata from
a clean build. The incremental one is 16 bytes longer.
`docs/hunt/p6-expansions` has it reduced to two crates and the derive:
about 60 lines of the library and 17 of the dependent.

What mirth's record shows, comparing the incremental process with the
clean one: the same tables are written, but only the incremental one, while
encoding, resolves foreign expansions (`expn_hash_to_expn_id`) and reads
foreign source files (`imported_source_file`, 150 times). So the
incremental session's metadata carries expansion and span data that the
clean session's does not. It needs the derive from the proc-macro crate:
without `#[derive(Named)]` it goes away. But so does removing the built-in
derives beside it, and, as with finding 1, removing unrelated items.

Syntax contexts restored from the incremental cache are the place to look,
near [#161450](https://github.com/rust-lang/rust/pull/161450), which
changed how syntax contexts are encoded. This was not taken further.

### 3. `impl Trait` in traits under `-Zthreads`

```rust
pub trait Render { fn render(&self) -> impl Sized; }
pub trait Draw { fn draw(&self) -> impl Sized; }
```

Eight builds with `-Zthreads=8` give three or four different `.rmeta` files.
A comment on [#162202](https://github.com/rust-lang/rust/issues/162202)
reports return-position `impl Trait` in traits as not reproducible, so this
is known. The fix for finding 1 does not change it. Both fixtures have such
traits, so P5 under `-Zthreads=8` fails on `wide` every time, and on
`chain` occasionally (1 build in 24).

## Not done

- Neither new finding has been reported upstream. Before filing them,
  check for duplicates again; #163878 especially is moving.
- Finding 2 is not root-caused.
- `fixtures/wide` fails `check.sh` (P5 under `-Zthreads=8`, finding 3) and
  P6 for most edits (findings 1 and 2) until those are fixed. Its lists are
  blessed against the unmodified compiler.

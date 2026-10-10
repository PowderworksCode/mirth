# Regressions in real crates, nightly-2026-07-18 to nightly-2026-10-06

Facts for findings 26 and 27. Found by the release-to-release check (`rustc/release-diff.py`,
[`checks.md`](../checks.md) check 2): `cargo check --locked` of 87 popular repositories
(`~/proofhouse-repos/rust`) under both nightlies. 53 behave the same, 18 fail on both, 10 could not
fetch their locked dependencies. Of the 5 regressions, two are `allocative 0.3.4`, which enables
`#![feature(never_type)]` when it detects a nightly and then conflicts with `Infallible` becoming
`type Infallible = !` (expected for a crate using unstable features). One is a timeout under load.
The other two are below.

## 26. Never-type fallback: a stable edition-2021 crate breaks with no warning beforehand

meilisearch (`crates/milli/src/update/new/indexer/mod.rs:432`, edition 2021) fails on
nightly-2026-10-06 with E0605: "non-primitive cast: `Result<(), _>` as `Result<!, Error>`".
Reduced to [`tests/never-fallback-cast.rs`](tests/never-fallback-cast.rs):

```rust
#[derive(Debug)] struct Error;
type Result<T, E = Error> = std::result::Result<T, E>;
fn run<F: FnOnce() -> R + Send, R: Send>(f: F) -> std::thread::Result<R> { Ok(f()) }
fn work() -> Result<()> {
    run(|| {
        Ok(()) as Result<_>
    })
    .unwrap()?;
    Ok(())
}
fn main() { work().unwrap(); }
```

| toolchain, edition 2021 | result |
|---|---|
| 1.98.0 | compiles; no warning, also with `-W rust-2024-compatibility -W dependency-on-unit-never-type-fallback` |
| nightly-2026-07-18 | compiles, no warning |
| nightly-2026-10-06 | E0605, `_` inferred as `!` |

Between the two nightlies the never-type fallback became `!` in edition 2021 too: the textbook
case (`if c { return } else { foo() }` with `foo<T: Default>`) is the deny-by-default lint "this
function depends on never type fallback being `()`" on 1.98 and the July nightly, and E0277
`!: Default` on the October nightly. That change is planned. The finding is that this program
depends on the fallback (the `_` in the cast target falls back through the `?` on the closure's
result) and the lint meant to announce the change says nothing about it. The code compiles warning-free
on stable and stops compiling. Not found in the issue tracker.

## 27. New trait solver: a type parameter reached only through a projection is not inferred

surrealdb fails through its dependency `diskann-wide 0.54.0`
(`src/arch/x86_64/v3/mod.rs:435`) with E0283 "type annotations needed". Reduced to
[`tests/next-solver-fn-ptr-projection.rs`](tests/next-solver-fn-ptr-projection.rs):

```rust
pub trait AddLifetime: 'static { type Of<'a>; }
pub trait FTarget1<A, R, T> { fn run(a: A, t: T) -> R; }
#[derive(Clone, Copy)] pub struct V;
impl V {
    pub unsafe fn run_function_with_1<F, T0, R>(self, x0: T0::Of<'_>) -> R
    where T0: AddLifetime, F: for<'a> FTarget1<Self, R, T0::Of<'a>> { F::run(self, x0) }

    pub fn dispatch1<F, R, T0>(self) -> unsafe fn(Self, T0::Of<'_>) -> R
    where T0: AddLifetime, F: for<'a> FTarget1<Self, R, T0::Of<'a>> {
        let f: unsafe fn(Self, T0::Of<'_>) -> R = Self::run_function_with_1::<F, _, _>;
        f
    }
}
```

| toolchain | result |
|---|---|
| 1.98.0, nightly-2026-07-18 | compiles |
| nightly-2026-07-18 `-Znext-solver=globally` | E0283 |
| nightly-2026-10-06 (new solver by default) | E0283 |
| nightly-2026-10-06 `-Znext-solver=coherence` | compiles |

`T0` appears in the function pointer type only as `T0::Of<'_>`. The old solver infers it (the
caller's where-clauses name the same projection); the new solver reports ambiguity. Since
#160895 made the new solver nightly's default, real crates hit it. The UI-test differences of the
same kind are in [`solver.md`](../solver.md).

**Known and intended:** #160895 lists `diskann-wide` under "higher-ranked associated type"
(trait-system-refactor-initiative#168: the old solver sometimes guided inference incorrectly when
relating higher-ranked associated types), 0.55 not yet patched. surrealdb, which depends on 0.54,
is not in that issue's list of affected crates.

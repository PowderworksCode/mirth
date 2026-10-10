# New trait solver: what mirth found, against what is already tracked

Everything mirth found that differs between the old trait solver and nightly's default (the new
solver everywhere, since #160895), checked against the tracking issue
[#160895](https://github.com/rust-lang/rust/issues/160895) (its "known impact" categories, the
affected-crates tables and the "unintended breakage" list), rust-lang/rust issues and
rust-lang/trait-system-refactor-initiative (tsri) issues, on 2026-10-10.

Sources: the UI-test solver differential ([`solver.md`](solver.md), `rustc/solver-diff.py`:
26 tests accepted by the old solver and rejected by the new, 3 crashes), the internal-checks sweep
(findings 22 and 23), release-to-release (finding 27).

## Severity

- **high**: stable code (no feature gates) rejected or crashing under nightly's default; it
  reaches stable users when the solver does
- **medium**: a documented behavior not implemented, or an unstable feature that stops working
  under the default solver
- **low**: incomplete features, debug-assertion builds only, or informational

## Summary

| # | what | tests | stable code? | tracked? | severity | worth reporting |
|---|---|---:|---|---|---|---|
| A | implied bounds through a higher-ranked supertrait projection (`gluon_salsa`): E0309 | 1 | yes | **no** | **high** | **yes**: a new issue, linked from #160895 |
| B | recursive `Peekable` instantiation: ICE `failed to resolve instance` | 3 | yes | [#152827](https://github.com/rust-lang/rust/issues/152827) (open) | high | a comment on #152827: three UI tests crash on default nightly |
| C | unconstrained RPIT (`fn test() -> impl Sized { test() }`): E0282 | 1 | yes | [tsri#144](https://github.com/rust-lang/trait-system-refactor-initiative/issues/144) (open) | high | no (known) |
| D | `-Zhigher-ranked-assumptions` does not fall back to the old solver as #160895 says | 12 | no (flag) | **no** | **medium** | **yes**: a comment on #160895 (the text or the code is wrong) |
| E | `TransmuteFrom` between references fails (`#![feature(transmutability)]`): E0277 | 8 | no | **no** | **medium** | **yes**: a new issue |
| F | higher-ranked associated type no longer guides inference (`escaping-bounds`; `diskann-wide`) | 1 + crate | yes | #160895, tsri#168: intended | low | surrealdb as an affected project on #160895 |
| G | type alias `impl Trait`: "does not constrain", a cycle | 2 | no | #160895: RPIT/TAIT handling changed | low | no |
| H | `fn_delegation` with `impl Trait` returns: E0282 | 1 | no (incomplete) | no | low | optional |
| 22 | debug assertion `!type_outlives.has_non_rigid_aliases()` in region outlives | 5 | some | sibling of closed #160206 | low | optional: debug builds only, but an invariant broken |
| 23 | integer overflow in `ty/instance.rs:421` (`recursion/issue-83150.rs`) | 1 | yes | no | low | optional |

## Details

### A. Implied bounds through a higher-ranked supertrait projection (high, untracked)

`tests/ui/implied-bounds/gluon_salsa.rs`, a reduction of the gluon and salsa crates, no feature
gates:

```rust
pub trait QueryBase { type Db; }
pub trait AsyncQueryFunction<'f>:
    QueryBase<Db = <Self as AsyncQueryFunction<'f>>::SendDb>
{ type SendDb; }
pub struct QueryTable<'me, Q, DB> { _q: Option<Q>, _db: Option<DB>, _marker: Option<&'me ()> }
impl<'me, Q> QueryTable<'me, Q, <Q as QueryBase>::Db>
where Q: for<'f> AsyncQueryFunction<'f>,
{ pub fn get_async<'a>(&'a mut self) { panic!(); } }
```

| toolchain | result |
|---|---|
| 1.98.0 | compiles |
| nightly-2026-07-18 `-Znext-solver=globally` | E0309: `<Q as AsyncQueryFunction<'_>>::SendDb` may not live long enough |
| nightly-2026-10-06 (default) | E0309 |
| nightly-2026-10-06 `-Znext-solver=coherence` | compiles |

The test exists to keep this pattern compiling (it came from real crates). It matches none of
#160895's categories.

### B. Recursive `Peekable` instantiation crashes (high, tracked)

`codegen/normalization-overflow/recursion-issue-{122823,131342,92004}.rs`: the tests expect
"reached the recursion limit"; under the default solver, an ICE "failed to resolve instance for
<&mut Peekable<…>>". Matches #152827 (open). Details in [`solver.md`](solver.md).

### C. Unconstrained RPIT (high, tracked)

`impl-trait/recursive-impl-trait-type-direct.rs`: `fn test() -> impl Sized { test() }` compiles
with the old solver, E0282 with the new. Tracked as tsri#144 (open).

### D. `-Zhigher-ranked-assumptions` does not fall back (medium, untracked)

#160895: "This unstable flag is also not supported with the new solver and when set, we're also
automatically falling back to the stable `-Znext-solver=coherence`." It does not:
`TyCtxt::next_trait_solver_globally` (`compiler/rustc_middle/src/ty/context.rs:2819`) falls back
only for `generic_const_exprs`:

```rust
    pub fn next_trait_solver_globally(self) -> bool {
        self.sess.opts.unstable_opts.next_solver == NextSolverConfig::Globally
            && !self.features().generic_const_exprs()
    }
```

The 12 UI tests whose `assumptions` revision passes `-Zhigher-ranked-assumptions` and expects
`check-pass` fail under the default solver ("higher-ranked lifetime error", "higher-ranked subtype
error", "lifetime bound not satisfied") and pass with `-Znext-solver=coherence` added:
`async-await/witness-auto-trait/higher-ranked-auto-trait-{1,4,5,6,8,9,12,15,17,18}.rs`,
`async-await/drop-tracking-unresolved-typeck-results.rs`,
`async-await/return-type-notation/issue-110963-early.rs`. CI does not see it, because compiletest
pins `-Znext-solver=coherence`.

### E. `TransmuteFrom` between references (medium, untracked)

Eight `check-pass` tests of `#![feature(transmutability)]`, all transmuting between references
(`&u8 → &Unit`, `&[u16; 0] → &[u8; 0]`, `&&u32 → &&i32`, recursive wrappers, `Assume` with
`lifetimes`/`safety`): E0277 "cannot be safely transmuted … unsatisfied trait bound" under the new
solver since at least July, accepted with `-Znext-solver=coherence`:
`transmutability/alignment/align-pass.rs`,
`transmutability/references/{accept_assume_lifetime_extension,recursive-wrapper-types,recursive-wrapper-types-bit-compatible,u8-to-unit,unit-to-itself}.rs`,
`transmutability/safety/assume/should_accept_if_ref_src_has_safety_invariant.rs`,
`transmutability/transmute-higher-ranked.rs`. Non-reference transmutability tests pass.

### F. Higher-ranked associated types (intended)

`borrowck/alias-liveness/escaping-bounds.rs` (E0283) and the crate `diskann-wide` (finding 27):
inference used to be guided through a higher-ranked projection; #160895 lists this as intended
breakage (tsri#168) and has `diskann-wide 0.55` in its table. surrealdb (through
`diskann-wide 0.54`) is not listed there.

### G, H. Unstable `impl Trait` features (low)

`impl-trait/recursive-type-alias-impl-trait-declaration-too-subtle-2.rs` ("item does not
constrain"), `type-alias-impl-trait/struct-assignment-validity.rs` (E0391 cycle),
`delegation/impl-trait.rs` (E0282, incomplete `fn_delegation`). #160895 says TAIT handling changed
substantially.

### 22, 23. Internal checks (low)

See [`hunt/internal-checks.md`](hunt/internal-checks.md): both need a debug-assertions or
overflow-checked compiler and the new solver.

## What to report, in order

1. A: a new issue (high, stable code, untracked).
2. E: a new issue (medium, a whole feature area under the default).
3. D: a comment on #160895 (the documented fallback does not exist).
4. B: a comment on #152827 (the crash is now on default nightly).
5. F: surrealdb on #160895's affected list.

As with all mirth findings, the reports are written by a person; this page is the facts.

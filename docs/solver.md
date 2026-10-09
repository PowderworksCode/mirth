# The UI suite under nightly's default trait solver

Since compiler-team MCP #1014 (rust-lang/rust#160895), nightly builds use the new trait solver
everywhere by default (`CFG_DEFAULT_NEXT_SOLVER_GLOBALLY`), while compiletest passes
`-Znext-solver=coherence` to every UI test, so the suite checks the old solver only. Code that
nightly users compile is therefore not what the suite checks.

`rustc/ui-solver-diff.py` compiles every UI test both ways with stock nightly-2026-10-06, the way
its `//@` headers say (tests needing auxiliary crates or another target, and tests that set
`-Znext-solver` themselves, are left out):

    rustc/ui-solver-diff.py --rustc <rustc> --tests <rust>/tests/ui --out <dir>

17,716 tests compiled both ways; **300 differ**: 271 fail either way with a different first error,
**26 compile under the pinned solver and fail under nightly's default**, and **3 crash under the
default** where the pinned solver reports the expected error.

## Crashes

| test | under nightly's default |
|---|---|
| `codegen/normalization-overflow/recursion-issue-122823.rs` | error: internal compiler error: … failed to resolve instance for <&mut Peekable<std::iter: |
| `codegen/normalization-overflow/recursion-issue-131342.rs` | error: internal compiler error: … failed to resolve instance for <&mut Peekable<&mut Peeka |
| `codegen/normalization-overflow/recursion-issue-92004.rs` | error: internal compiler error: … failed to resolve instance for <&mut Peekable<std::iter: |

`failed to resolve instance` while instantiating a recursive `Peekable` chain, where the tests
expect "reached the recursion limit". The new solver crashed on these in July and August too
(with `-Znext-solver=globally`); what changed between nightly-2026-08-15 and nightly-2026-10-06
is the default, so every nightly user now gets the crash. Found first by the UI coverage run
([`coverage.md`](coverage.md)), as a test that crashes outside compiletest and passes inside it.

## Compile under the pinned solver, fail under the default

| test | first error under nightly's default |
|---|---|
| `async-await/drop-tracking-unresolved-typeck-results.rs` | error: lifetime bound not satisfied |
| `async-await/return-type-notation/issue-110963-early.rs` | error: lifetime bound not satisfied |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-1.rs` | error: higher-ranked subtype error |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-12.rs` | error: higher-ranked lifetime error |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-15.rs` | error: lifetime bound not satisfied |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-17.rs` | error: higher-ranked subtype error |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-18.rs` | error: higher-ranked lifetime error |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-4.rs` | error: higher-ranked subtype error |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-5.rs` | error: higher-ranked lifetime error |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-6.rs` | error: lifetime bound not satisfied |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-8.rs` | error: higher-ranked lifetime error |
| `async-await/witness-auto-trait/higher-ranked-auto-trait-9.rs` | error: higher-ranked lifetime error |
| `borrowck/alias-liveness/escaping-bounds.rs` | error[E0283]: type annotations needed |
| `delegation/impl-trait.rs` | error[E0282]: type annotations needed |
| `impl-trait/recursive-impl-trait-type-direct.rs` | error[E0282]: type annotations needed |
| `impl-trait/recursive-type-alias-impl-trait-declaration-too-subtle-2.rs` | error: item does not constrain … |
| `implied-bounds/gluon_salsa.rs` | error[E0309]: the associated type … may not live long enough |
| `transmutability/alignment/align-pass.rs` | error[E0277]: … cannot be safely transmuted into … |
| `transmutability/references/accept_assume_lifetime_extension.rs` | error[E0277]: … cannot be safely transmuted into … |
| `transmutability/references/recursive-wrapper-types-bit-compatible.rs` | error[E0277]: … cannot be safely transmuted into … |
| `transmutability/references/recursive-wrapper-types.rs` | error[E0277]: … cannot be safely transmuted into … |
| `transmutability/references/u8-to-unit.rs` | error[E0277]: … cannot be safely transmuted into … |
| `transmutability/references/unit-to-itself.rs` | error[E0277]: … cannot be safely transmuted into … |
| `transmutability/safety/assume/should_accept_if_ref_src_has_safety_invariant.rs` | error[E0277]: … cannot be safely transmuted into … |
| `transmutability/transmute-higher-ranked.rs` | error[E0277]: … cannot be safely transmuted into … |
| `type-alias-impl-trait/struct-assignment-validity.rs` | error[E0391]: cycle detected when computing type of … |

These look like the fallout #160895 tracks (higher-ranked auto traits of async blocks,
transmutability, type annotations); each is for someone working on the solver to sort into known
and new. The list is facts, not reports.

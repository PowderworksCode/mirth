# Machine-applicable lint fixes that break builds

Facts for finding 29. Found by the suggestions-apply check (`mirth-lab suggest-diff`, check 18 in
[`checks.md`](../checks.md)): every `MachineApplicable` suggestion of every UI test without
`//@ run-rustfix` (17,945 tests, 7,385 suggestions), each applied alone and compiled again. A
lint's machine-applicable fix is what `cargo fix` and `cargo clippy --fix` apply without asking;
a lint never stops a build, so its fix must not introduce an error. 111 lint fixes did.

## Six shapes, reduced, on stable 1.98.0

Each file in [`tests/lint-fixes/`](tests/lint-fixes/) compiles with warnings; applying the one
named suggestion (`apply-one-suggestion.py <file> 1.98.0`) gives the error shown. The same on
nightly-2026-10-06.

| file | lint | code | suggestion | after applying it |
|---|---|---|---|---|
| `ref.rs` | `unused_variables` | `let ref b = u; drop(u);` | `ref b` → `_b` | E0382 use of moved value: the binding now moves |
| `update.rs` | `unused_variables` | `fn test(f: Foo) { Foo { foo: 4, ..f } }` (every field given) | `f` → `_f` | E0425 cannot find value `f`: the base still names it |
| `unreach.rs` | `unused_variables` | `let x = f(); let _ = x;` with `f() -> Never` | `x` → `_x` | E0425: the later (unreachable) use still names `x` |
| `closure.rs` | `unused_variables` | `let mut x = 0; to_fn(move \|\| { x = 42; })` | `mut x` → `_x` | E0425: the closure still names `x` |
| `closure.rs` | `unused_mut` | the same | remove `mut` | E0594 cannot assign to `x`: the closure assigns |
| `ormut.rs` | `unused_mut` | `Ok(mut y) \| &Err(mut y) => drop(y)` | remove one `mut` | E0409 bound inconsistently across alternatives |
| `glob.rs` | `unused_imports` | `mod one_private { use crate::m::*; pub use crate::m::*; } use crate::one_private::S;` | remove `pub use crate::m::*;` | E0603 struct import `S` is private |

In the UI tests the same shapes appear as: `ref`/`ref mut` bindings, including `ref x @ pat` and
unsized `ref rest @ ..`, where E0277 follows (25 suggestions); variables mentioned again only in
unreachable code, struct-update bases or closures (48); or-patterns where one alternative's
`mut` or name is changed alone (12); glob re-exports that take part in ambiguity or visibility
(5).

## Expected

A machine-applicable suggestion keeps the program compiling, with the same meaning:
`ref _b` (or `_`) for a `ref` binding, renaming every mention or not offering the rename where
the variable is mentioned elsewhere, removing `mut` from every alternative, and not calling an
import unused when removing it changes resolution.

## Also found, lower priority

- Lint suggestions inside macro input that the macro then fails to match: `unexpected_cfgs`
  (`FALSE` → `false` in a macro's meta argument), `missing_abi` next to a literal with a suffix,
  `unused_parens` around `let` chains passed to a macro.
- 106 error suggestions that leave the same error, and 63 whose fix no longer parses, mostly in
  parser-recovery and `fn_delegation` tests (error-recovery suggestions on deliberately broken
  code).
- An ICE after applying an E0308 fix (`consts/const-eval/array-len-mismatch-type.rs`, stable
  1.98): const evaluation runs on a body that failed with E0277 and panics with "expected wide
  pointer extra data"; that ICE family has open #154779.

Not found in the issue tracker (searched for each shape).

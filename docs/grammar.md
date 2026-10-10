# Covering the grammar

`fixtures/sink` is the code the fuzzer, the flag walks and the history of findings run on, so
what it does not contain is never tested. `mirth-lab grammar-coverage` measures it against
[Ur](https://github.com/PowderworksCode/codebase/tree/main/projects/ur)'s Rust grammar
(`ecosystems/rust/language/*.rsc`):

    target/release/mirth-lab grammar-coverage --ur <ur> --grammar <ur>/ecosystems/rust/language fixtures/sink

It parses every `.rs` file with `ur parse --tree` and counts two things:

- **constructs**: each labeled alternative of each syntax rule (`Pattern::Range`,
  `Operation::CompoundAssign`, `ItemKind::TraitAlias`, …), by label within its module.
  `Conditions.rsc` repeats the expression rules for condition position (no struct literals);
  those count as the expressions they are.
- **literals**: each keyword and operator the syntax rules mention.

| | constructs | literals |
|---|---|---|
| sink before | 137 of 201 | 75 of 115 |
| with `core/src/grammar.rs` (stable) | 166 | 88 |
| with `nightly/` and `old/` | 200 | 108 |

What was added:

- `core/src/grammar.rs`, stable syntax sink did not use: bit operators and every compound
  assignment, `continue` and labelled `continue`, an empty statement, open ranges, `_ =`
  destructuring assignment, `&raw const`, a leading `|`, negative literal and range-bound
  patterns, constant and macro patterns, `ref`/`ref mut`, raw pointers, `fn` pointer types with
  qualifiers and C variadics, `!`, a variadic foreign function, qualified paths, associated type
  bounds, a generic associated type in a bound, negative and block const arguments, a macro in
  type position, `use<..>` precise capturing, and every reserved keyword as a raw identifier.
- `nightly/` (`sink-nightly`), syntax behind feature gates: expression attributes, `become`,
  `builtin #` (`offset_of`, `type_ascribe`, `deref`, `field_of`, `wrap_binder`,
  `unwrap_binder`), `gen` blocks and `.yield`, coroutines and `yield`, `&pin mut` (expression
  and type), postfix `.match`, `.use`, `try` and `try bikeshed` blocks, `do yeet`, unsafe
  binders, `move(..)`, contracts (`contract_requires`, `contract_ensures`), item-level
  `const { }`, `default fn` (specialization), `final fn`, `reuse` delegation (one function, a
  list, a glob, `reuse impl`), `macro` (macros 2.0), trait aliases, auto traits and negative
  impls, `mut(crate)` field restrictions, deref, guard and never patterns, return-type notation
  (in a bound and in a `where` clause), `const { }` as a generic argument, `const trait`,
  `const impl` and `~const`.
- `old/` (`sink-old`), edition 2015: anonymous trait parameters, bare trait objects, and
  `async`, `await`, `dyn`, `try` as identifiers.

`main` checks each, 84 checks in all.

Not covered, and why:

- `PathSegment::ReturnTypeNotation` in an expression path: rustc has no such form;
  `T::fetch(..)` in an expression is a call with a `..` argument.
- The literals `abstract`, `box`, `override`, `priv`, `typeof`, `unsized`, `virtual`: reserved
  keywords, usable only as raw identifiers, which sink has (`r#abstract`, …) but Ur records as
  names.

Found on the way: guard patterns ignore their guard (finding 15 in [`hunt.md`](hunt.md));
`builtin # offset_of` outside a `const` block is an ICE, which rustc says is expected of an
internal feature used directly.

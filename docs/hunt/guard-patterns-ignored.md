# Finding 15: guard patterns ignore their guard

Facts for a report; the report itself is for a person to write (rust-lang/rust's LLM policy).
`guard_patterns` is an incomplete feature (tracking issue #129967).

**Repro.** `docs/hunt/repro.sh`, "guard-patterns":

    #![feature(guard_patterns)]
    #![allow(incomplete_features)]
    fn f(o: Option<u32>) -> u32 {
        match o {
            Some(x if x > 3) => 4,
            Some(_) => 1,
            None => 2,
        }
    }
    fn main() { println!("{} {}", f(Some(9)), f(Some(2))); }

**Expected.** `4 1`: for `Some(2)` the guard `x > 3` fails and the second arm matches.

**Actual.** `4 4`, with a warning that `Some(_)` is an unreachable pattern because
`Some(x if x > 3)` "matches all the relevant values". The guard is dropped from both the
exhaustiveness check and the generated code.

Also: using the binding in the arm's body (`Some(x if x > 3) => x`) is rejected with
E0381 "used binding `x` isn't initialized", and an or-pattern of two guard patterns binding
the same name (`Some(x if x > 3) | Some(x if x == 0) => x`) gives the same error.

**Versions.** nightly-2026-07-18 and nightly-2026-10-06 (and the local compiler).

**How mirth found it.** Writing nightly syntax into `fixtures/sink` to cover every
alternative of Ur's Rust grammar (`rustc/grammar-coverage.py`); the fixture's runtime check.

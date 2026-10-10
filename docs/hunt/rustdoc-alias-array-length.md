# rustdoc evaluates an array length rustc never evaluates (type aliases)

Facts for finding 37. Found by the rustdoc consistency check (`mirth-lab rustdoc-diff`) on
`type-alias/lack-of-wfcheck.rs` and `type-alias/lack-of-wfcheck-generic-const-args.rs`
(check-pass).

## What happens

```rust
type Diverging = [(); panic!()];
fn main() {}
```

`rustc` compiles it (type aliases are not checked for well-formedness, and nothing uses the
alias). `rustdoc` fails:

```
error[E0080]: evaluation panicked: explicit panic
```

The same for `pub type Diverging = [(); panic!()];` in a library, with 1.80.0 ("evaluation of
constant value failed"), 1.90.0, 1.98.0 and nightly-2026-10-06.

## Where

`src/librustdoc/clean/mod.rs`, `clean_ty` (line 1925), for an array type whose length is an
anonymous constant (line 1963):

```rust
                    let ct = lower_const_arg_for_rustdoc(cx.tcx, const_arg, cx.tcx.types.usize);
                    let typing_env = ty::TypingEnv::post_analysis(cx.tcx, *def_id);
                    let ct =
                        cx.tcx.normalize_erasing_regions(typing_env, Unnormalized::new_wip(ct));
```

Normalizing evaluates the constant (`const_eval_resolve_for_typeck`), and a failed evaluation
reports E0080 (backtrace under `-Ztreat-err-as-bug=1`: `rustdoc::clean::clean_ty` →
`normalize_erasing_regions` → `eval_to_valtree` → `report_eval_error`). rustc evaluates the
length only where the alias is used.

## Scope

Documenting a crate with a type alias whose array length fails to evaluate (panics, overflows,
indexes out of bounds) and that the crate never uses: `cargo doc` fails although `cargo build`
succeeds. Low. Not found in the issue tracker (searched 2026-10-10).

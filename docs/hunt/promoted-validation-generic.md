# An invalid constant is accepted when its unused reference sits in a generic function

Facts for finding 25. Found by the equivalent-rewrite differential (`rustc/rewrite-diff.py`, the
`generic-wrap` rewrite: a function's body moved into a generic inner function called with
`()`) on `tests/ui/consts/interior-mut-const-via-union.rs`.

## What happens

The test builds a constant `C: S` whose enum tag is changed through a raw pointer, so that it
holds an `UnsafeCell` (through a union). Taking `&C` is rejected with E0080 ("constructing invalid
value of type &S: … encountered `UnsafeCell` in read-only memory"). Whether that error is reported
depends on where the reference is taken and on the MIR optimization level:

| `C` as in the test; then | rustc (pinned nightly, also 1.80.0, 1.90.0, 1.98.0) |
|---|---|
| `fn main() { let _: &'static _ = &C; }` | E0080 |
| `fn inner() { let _: &'static _ = &C; } fn main() { inner() }` | E0080 |
| `fn inner<T>() { let _: &'static _ = &C; } fn main() { inner::<()>() }` | **compiles** |
| the same, `-Zmir-opt-level=0` | E0080 |
| the same with `let _x: &'static S = &C;` | E0080 |
| `fn inner<T>() -> &'static S { &C }` used by `main` | E0080 |

`-Copt-level` makes no difference. The program is the same in every row: the reference is taken
in a function that is instantiated and called.

## Where it goes

`-Zdump-mir=inner` at `-Zmir-opt-level=1`. Up to `LowerIntrinsics` the body has
`_4 = const inner::<T>::promoted[0]; _2 = &(*_4); _1 = &(*_2); PlaceMention(_1);`.
`RemovePlaceMention` drops the mention, `SimplifyCfg-pre-optimizations` and
`InstSimplify-before-inline` simplify the copies, and `SimplifyLocals-before-const-prop` removes
the last statement using `promoted[0]`. A non-generic body's promoteds are evaluated during
analysis, before any of this. A generic body's promoted is evaluated at monomorphization: when
code generation uses it (`_x`, the returned reference) it is validated and the error appears. When
the use has been optimized away, nothing reports the error, although the promoted should still be
evaluated as a required constant of the body. Not narrowed further: whether the required-constant
evaluation runs without validation, or the promoted is missing from `required_consts`.

## Expected

The same verdict in every row. Either the invalid value is an error wherever it is referenced
(as in the non-generic and `-Zmir-opt-level=0` cases), or nowhere. The MIR optimization level and
whether a function is generic should not decide whether a program compiles.

## Reproduction

```rust
use std::cell::Cell;
use std::mem::ManuallyDrop;

#[repr(C)] struct S { x: u32, y: E }
#[repr(u32)] enum E { A, B(U) }
union U { cell: ManuallyDrop<Cell<u32>> }

const C: S = {
    let mut s = S { x: 0, y: E::A };
    let p = &mut s.x as *mut u32;
    unsafe { *p.add(1) = 1 };
    s
};

fn inner<T>() { let _: &'static _ = &C; }
fn main() { inner::<()>() }
```

`rustc generic.rs` compiles. `rustc -Zmir-opt-level=0 generic.rs` gives E0080. Without the type
parameter it gives E0080 at every level.

## Local stopgap

None; not an incremental difference. `rewrite-diff.py` lists the test as known for `generic-wrap`.

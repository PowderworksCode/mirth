# gdb pretty-printers: zero-sized elements, and Ref/RefMut

Facts for findings 48 and 49. Found by the debugger round trip ([`checks.md`](../checks.md),
check 20: `mirth-lab debug-check`): generated programs build known values, gdb prints them at a
breakpoint, and the output is compared with the values. gdb 15.1 (Ubuntu 24.04), the Rust
pretty-printers shipped with the toolchain (`lib/rustlib/etc/gdb_providers.py`), loaded by
`rust-gdb`.

## 48. Collections of zero-sized elements

[`tests/debug-check/zst-printers.rs`](tests/debug-check/zst-printers.rs),
`rustc --edition 2021 -g`, then `print` at the breakpoint (pinned nightly):

| value | gdb prints |
|---|---|
| `Vec<Z>` with two elements (`struct Z;`) | `Vec(size=2)Python Exception <class 'gdb.error'>: Cannot perform pointer math on incomplete type "zst_printers::Z", try casting to a known type, or void *.` |
| `VecDeque<()>` with two elements | `VecDeque(size=2)Python Exception <class 'ZeroDivisionError'>: integer modulo by zero` |
| `&[Z]` with three elements | `&[zst_printers::Z](size=3)Python Exception <class 'gdb.error'>: Cannot perform pointer math on incomplete type …` |
| `BTreeMap<u8, One>` with `(1, One::Only)` (`enum One { Only }`) | `BTreeMap(size=1) = {[1] = ()}` |

The elements are missing (with an exception) for Vec, VecDeque and slices; the BTreeMap value
is shown as `()`, which is not its type. The same happens nested (a Vec of a VecDeque of
`PhantomData`, a HashMap whose values are such VecDeques, …).

Where, in `src/etc/gdb_providers.py`:

- `StdVecProvider.children` and `StdSliceProvider.children`: `self._data_ptr + index`. gdb
  refuses pointer arithmetic on a pointer to a type of size 0 ("incomplete type").
- `StdVecDequeProvider`: `self._cap = int(cap)` reads `buf.inner.cap`, which `RawVec` keeps at
  0 for a zero-sized `T` (its `capacity()` reports `usize::MAX` without storing it), and
  `children` computes `(self._head + index) % self._cap`.
- `StdBTreeMapProvider` (`children_of_node`) already avoids the pointer arithmetic on
  zero-sized keys and values, with the comment "Avoid "Cannot perform pointer math on incomplete
  type" on zero-sized arrays", by yielding `gdb.parse_and_eval("()")` instead of the key or
  value. That is right for `()` and wrong for every other zero-sized type (unit structs,
  one-variant enums, `PhantomData`). `tests/debuginfo/pretty-std-collections.rs` covers it with
  `BTreeMap<(), i32>` and `BTreeMap<i32, ()>` only.

No test in `tests/debuginfo` prints a Vec, VecDeque or slice of zero-sized elements under gdb.

Versions: the Vec, VecDeque and slice exceptions are the same with 1.80.0, 1.90.0, 1.98.0,
nightly-2026-07-18 and nightly-2026-10-06 (each with its own `rust-gdb` and printers).

## 49. Ref and RefMut

[`tests/debug-check/ref-printer.rs`](tests/debug-check/ref-printer.rs): a live `RefCell` borrow
(`let r = a.borrow();`, `let w = b.borrow_mut();`), printed at the breakpoint:

```
$1 = Python Exception <class 'gdb.error'>: Attempt to take contents of a non-pointer value.
Python Exception <class 'gdb.error'>: Attempt to take contents of a non-pointer value.
core::cell::Ref<u8> {value: core::ptr::non_null::NonNull<u8> {pointer: 0x7fffffffdb58}, borrow: core::cell::BorrowRef {borrow: 0x7fffffffdb50}}
$2 = Python Exception <class 'gdb.error'>: Attempt to take contents of a non-pointer value.
…
core::cell::RefMut<alloc::string::String> {value: core::ptr::non_null::NonNull<alloc::string::String> {pointer: 0x7fffffffdb78}, …}
```

Every `Ref` and `RefMut` fails; gdb falls back to the raw struct, so neither the borrowed value
nor the borrow count is shown.

Where: `StdRefProvider.__init__` does `valobj["value"].dereference()`. `core::cell::Ref::value`
and `RefMut::value` are `NonNull<T>` (a struct with a `pointer` field), not a pointer, so
`dereference()` raises. The other providers unwrap `NonNull` with `unwrap_unique_or_non_null`.
The next line, `valobj["borrow"]["borrow"]["value"]["value"]`, reads through
`BorrowRef::borrow`, which is a `&Cell<BorrowCounter>` (a pointer). Not checked whether it would
work once the first line is fixed.

`tests/debuginfo/mutable-locs.rs` prints `Ref` and `RefMut` under cdb only; no gdb test prints
either.

Versions: the same exception with 1.80.0, 1.90.0, 1.98.0, nightly-2026-07-18 and
nightly-2026-10-06.

## Searched

rust-lang/rust issues for "gdb VecDeque", "pretty printer zero-sized", "Cannot perform pointer
math", "ZeroDivisionError gdb_providers", "gdb Ref pretty printer", "RefMut gdb",
"StdRefProvider", "Attempt to take contents of a non-pointer value": nothing about these
(#55944, closed, is an older VecDeque printer bug).

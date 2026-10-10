#![feature(dropck_eyepatch)]
#![warn(single_use_lifetimes)]

struct Pr<'a, T>(&'a T);

// `'a` is used once, so the lint suggests `'_`. The machine-applicable fix deletes `'a, ` but not
// its attribute, giving `unsafe impl<#[may_dangle] T> Drop for Pr<'_, T>`, which compiles: the
// unsafe promise moved from `'a` to `T`. (In tests/ui/drop/dropck-eyepatch-reorder.rs the
// attribute lands where it is rejected instead: E0199.)
unsafe impl<#[may_dangle] 'a, T> Drop for Pr<'a, T> {
    fn drop(&mut self) {}
}

fn main() {}

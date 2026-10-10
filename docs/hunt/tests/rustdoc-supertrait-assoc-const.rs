#![feature(gca_min_const_items)]
use std::gca;
trait B {
    #[rustc_always_gca]
    const CONST: i32;
}
trait C: B {}
fn generic<T: C<CONST = 2>>() {}
fn main() {}

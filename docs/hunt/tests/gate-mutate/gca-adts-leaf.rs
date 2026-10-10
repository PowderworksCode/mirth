#![feature(gca_adts, gca_macroless_items, gca_min_const_items)]
const TUPLE: (&'static str, &'static str) = ("a", true);
fn main() { TUPLE; }

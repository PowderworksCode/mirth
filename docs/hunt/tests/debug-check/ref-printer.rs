// Finding 49: the gdb pretty-printer for core::cell::Ref and RefMut.
//   rustc -g ref-printer.rs && rust-gdb -batch -ex 'break ref_printer::bp' -ex run -ex up \
//     -ex 'print r' -ex 'print w' ./ref-printer
use std::cell::RefCell;

#[inline(never)]
fn bp() {
    std::hint::black_box(());
}

fn main() {
    let a = RefCell::new(5u8);
    let r = a.borrow();
    let b = RefCell::new(String::from("x"));
    let w = b.borrow_mut();
    bp();
    std::hint::black_box((&r, &w));
}

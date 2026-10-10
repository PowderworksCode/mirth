#![warn(let_underscore_drop)]
use std::cell::RefCell;

const A: RefCell<u8> = RefCell::new(0);

fn main() {
    let _ = A.borrow();
}

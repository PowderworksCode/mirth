//! The bottom of the chain: a constant, a trait with a default method, a
//! struct and its impl, a generic function, an `#[inline]` function and a
//! deprecated one, an async closure, a trait returning `impl Trait` and an
//! exported symbol, for dependents to read.

pub const LIMIT: u32 = 100;

pub trait Shape {
    fn area(&self) -> u32;

    fn describe(&self) -> String {
        format!("area {}", self.area())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Square(pub u32);

impl Shape for Square {
    fn area(&self) -> u32 {
        self.0 * self.0
    }
}

pub fn largest<T: Shape>(shapes: &[T]) -> Option<&T> {
    shapes.iter().max_by_key(|shape| shape.area())
}

#[inline]
pub fn clamp(value: u32) -> u32 {
    value.min(LIMIT)
}

#[deprecated(note = "use `clamp`")]
pub fn old_clamp(value: u32) -> u32 {
    clamp(value)
}

/// An async closure that borrows what it captures, so calling it by value
/// needs the separate by-move body rustc makes for it (#130201).
pub fn greeter(name: String) -> impl AsyncFn() -> usize {
    async move || name.len()
}

/// A trait whose method returns `impl Trait`, so rustc creates definitions
/// for the hidden types while it analyses the crate (#162910).
pub trait Label {
    fn label(&self) -> impl std::fmt::Display;
}

impl Label for Square {
    fn label(&self) -> impl std::fmt::Display {
        format!("square {}", self.0)
    }
}

/// An exported symbol; rustdoc reads its `#[no_mangle]` from metadata (#144050).
#[unsafe(no_mangle)]
pub extern "C" fn base_limit() -> u32 {
    LIMIT
}

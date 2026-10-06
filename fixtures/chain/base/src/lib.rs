//! The bottom of the chain: a constant, a trait with a default method, a
//! struct and its impl, a generic function, an `#[inline]` function and a
//! deprecated one, for dependents to read.

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

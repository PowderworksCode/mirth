//! The middle: implements a trait from `base` and re-exports from it.

pub use base::Square;
use base::Shape;

pub struct Rectangle {
    pub width: u32,
    pub height: u32,
}

impl Shape for Rectangle {
    fn area(&self) -> u32 {
        base::clamp(self.width * self.height)
    }
}

/// A type whose size is computed from `base`'s constant, so it changes when
/// that constant does.
pub type Buffer = [u8; base::LIMIT as usize];

pub fn total<T: Shape>(shapes: &[T]) -> u32 {
    shapes.iter().map(Shape::area).sum()
}

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

pub fn total<T: Shape>(shapes: &[T]) -> u32 {
    shapes.iter().map(Shape::area).sum()
}

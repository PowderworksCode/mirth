pub use wide_core::{Point, Shape, poly};
use wide_derive::{Named, counted};
#[derive(Named, Debug, Clone, PartialEq)]
pub struct Drawing {
    pub shapes: Vec<Shape>,
}
pub fn each_point(drawing: &Drawing, mut f: impl FnMut(Point)) {
    for shape in &drawing.shapes {
        match shape {
            Shape::Dot(p) => f(*p),
            Shape::Line { from, to } => {
            }
            Shape::Poly(points) => points.iter().copied().for_each(&mut f),
            Shape::Empty => {}
        }
    }
}

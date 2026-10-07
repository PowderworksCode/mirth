//! The middle crate: uses the derive, the attribute macro and the exported
//! macro, implements `wide-core`'s traits, and re-exports some of it.

pub use wide_core::{Point, Shape, poly};
use wide_core::{Named, Render, Windows};
use wide_derive::{Named, counted};

#[derive(Named, Debug, Clone, PartialEq)]
pub struct Drawing {
    pub shapes: Vec<Shape>,
}

#[derive(Named, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Layer {
    Back,
    Middle,
    Front,
}

impl Drawing {
    #[counted]
    pub fn sample() -> Drawing {
        Drawing { shapes: vec![poly![(1, 2), (3, 4), (5, 6)], Shape::Dot(wide_core::ORIGIN)] }
    }

    pub fn describe(&self) -> Vec<String> {
        self.shapes.iter().map(|shape| shape.render().to_string()).collect()
    }

    pub fn pairs(&self) -> usize {
        let names: Vec<&str> = self.shapes.iter().map(|_| self.name()).collect();
        (0..names.len()).filter_map(|i| names.window(i)).count()
    }
}

pub type Canvas = wide_core::Grid<u8, 4, 3>;

pub fn canvas() -> Canvas {
    Canvas::filled(wide_core::square(3) as u8)
}

pub fn layers() -> impl Iterator<Item = (Layer, &'static str)> {
    [Layer::Back, Layer::Middle, Layer::Front].into_iter().map(|l| (l, Layer::NAME))
}

/// Calls back into a closure the caller provides; generic over it.
pub fn each_point(drawing: &Drawing, mut f: impl FnMut(Point)) {
    for shape in &drawing.shapes {
        match shape {
            Shape::Dot(p) => f(*p),
            Shape::Line { from, to } => {
                f(*from);
                f(*to)
            }
            Shape::Poly(points) => points.iter().copied().for_each(&mut f),
        }
    }
}

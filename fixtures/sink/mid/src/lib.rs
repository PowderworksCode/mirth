//! The middle: implements `sink-core`'s traits, uses all three kinds of
//! procedural macro and the exported declarative macros, re-exports, and
//! generic code that dependents instantiate.

pub use sink_core::shapes::{Container, Labels, V2};
pub use sink_core::{Area, Describe, Shape, Square, sum};

use sink_core::asyncs::Fetch;
use sink_macros::{Describe, squares, traced};

pub mod nested {
    pub mod deeper {
        pub(crate) fn hidden() -> u8 {
            7
        }
        pub(in crate::nested) fn semi() -> u8 {
            super::super::nested::deeper::hidden() + 1
        }
        pub fn visible() -> u8 {
            semi() * 2
        }
    }
    pub use self::deeper::visible as shown;
}

/// A shape defined downstream, with a derive and a helper attribute.
#[derive(Debug, Clone, PartialEq, Describe)]
#[describe(tag = "polygon")]
pub struct Polygon {
    pub sides: u32,
    pub length: f64,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Describe)]
pub enum Colour {
    Red,
    Green,
    Blue,
}

impl Area for Polygon {
    fn area(&self) -> f64 {
        let n = f64::from(self.sides);
        n * self.length * self.length / (4.0 * (core::f64::consts::PI / n).tan())
    }
}

impl Shape for Polygon {
    type Unit = f64;

    fn name(&self) -> &'static str {
        self.label
    }

    fn perimeter(&self) -> f64 {
        f64::from(self.sides) * self.length
    }
}

#[traced]
pub fn traced_square(x: i64) -> i64 {
    x * x
}

pub fn squared_table() -> [i64; 4] {
    squares!(1, 2, 3, 4)
}

pub fn macro_sum() -> i32 {
    sum!(1, 2, 3, 4, 5)
}

pub fn shadowing() -> i32 {
    let x = 1;
    sink_core::shadow!(x)
}

/// Generic code that dependents instantiate, with `#[inline]` so its MIR is in the metadata.
#[inline]
pub fn largest<S: Shape + ?Sized>(shapes: &[&S]) -> Option<f64> {
    shapes.iter().map(|s| s.area()).reduce(f64::max)
}

pub struct Wrapper<T, const N: usize> {
    pub items: [T; N],
}

impl<T: Clone + Default + core::fmt::Debug, const N: usize> Wrapper<T, N> {
    pub fn filled(t: T) -> Self {
        Wrapper { items: core::array::from_fn(|_| t.clone()) }
    }

    pub fn describe(&self) -> String {
        format!("{} items: {:?}", N, self.items)
    }
}

pub async fn lookup(store: &impl Fetch, keys: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for key in keys {
        if let Some(v) = store.fetch(key).await {
            out.push(v);
        }
    }
    out
}

pub fn labels(squares: &[Square]) -> Vec<String> {
    squares.labels().collect()
}

pub fn colours() -> Vec<(&'static str, &'static str)> {
    [Colour::Red, Colour::Green, Colour::Blue].iter().map(|c| (Colour::NAME, c.tag())).collect()
}

//! The bottom crate: derives, generics of every kind, an exported macro,
//! a generated table, statics, and traits for dependents to implement.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::AtomicUsize;

include!(concat!(env!("OUT_DIR"), "/table.rs"));

pub static COUNT: AtomicUsize = AtomicUsize::new(0);

pub trait Named {
    const NAME: &'static str;

    fn name(&self) -> &'static str {
        Self::NAME
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Shape {
    Dot(Point),
    Line { from: Point, to: Point },
    Poly(Vec<Point>),
}

#[derive(Debug, Clone)]
pub struct Grid<T, const W: usize, const H: usize> {
    pub cells: [[T; W]; H],
}

impl<T: Copy + Default, const W: usize, const H: usize> Grid<T, W, H> {
    pub fn filled(value: T) -> Self {
        Grid { cells: [[value; W]; H] }
    }

    pub const AREA: usize = W * H;
}

/// A lending iterator: a generic associated type.
pub trait Windows {
    type Window<'a>
    where
        Self: 'a;

    fn window(&self, at: usize) -> Option<Self::Window<'_>>;
}

impl<T> Windows for Vec<T> {
    type Window<'a>
        = &'a [T]
    where
        T: 'a;

    fn window(&self, at: usize) -> Option<&[T]> {
        self.get(at..at + 2)
    }
}

pub trait Render {
    fn render(&self) -> impl fmt::Display + '_;

    fn boxed(&self) -> Box<dyn fmt::Display + '_> {
        Box::new(self.render())
    }
}

impl Render for Shape {
    fn render(&self) -> impl fmt::Display + '_ {
        match self {
            Shape::Dot(p) => format!("dot {},{}", p.x, p.y),
            Shape::Line { from, to } => format!("line {from:?} {to:?}"),
            Shape::Poly(points) => format!("poly of {}", points.len()),
        }
    }
}

#[allow(async_fn_in_trait)]
pub trait Store {
    async fn load(&self, key: &str) -> Option<u32>;
}

pub struct Memory(pub BTreeMap<String, u32>);

impl Store for Memory {
    async fn load(&self, key: &str) -> Option<u32> {
        self.0.get(key).copied()
    }
}

/// Builds a `Shape::Poly` from a list of pairs.
#[macro_export]
macro_rules! poly {
    ($(($x:expr, $y:expr)),* $(,)?) => {
        $crate::Shape::Poly(vec![$($crate::Point { x: $x, y: $y }),*])
    };
}

pub fn spread<I>(points: I) -> i32
where
    I: IntoIterator<Item = Point>,
{
    let (min, max) = points
        .into_iter()
        .fold((i32::MAX, i32::MIN), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
    max - min
}

#[inline]
pub fn square(i: usize) -> u32 {
    SQUARES[i % SQUARES.len()]
}

pub const ORIGIN: Point = Point { x: 0, y: 0 };

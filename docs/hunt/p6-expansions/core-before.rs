use std::fmt;
pub trait Named {
    const NAME: &'static str;
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
#[macro_export]
macro_rules! poly {
    ($(($x:expr, $y:expr)),* $(,)?) => {
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

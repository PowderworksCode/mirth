//! Traits with associated types and consts, default methods, supertraits,
//! generic and blanket impls, trait objects and upcasting, operators.

use core::fmt;
use core::ops::{Add, Index, Mul};

pub trait Area {
    fn area(&self) -> f64;
}

pub trait Shape: Area + fmt::Debug {
    type Unit: Copy + fmt::Display;

    fn name(&self) -> &'static str;

    fn perimeter(&self) -> f64 {
        0.0
    }

    fn scaled(&self, by: f64) -> Box<dyn Shape<Unit = Self::Unit>>
    where
        Self: Sized + Clone + 'static,
    {
        let _ = by;
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Square {
    pub side: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Circle {
    pub radius: f64,
}

impl Area for Square {
    fn area(&self) -> f64 {
        self.side * self.side
    }
}

impl Area for Circle {
    fn area(&self) -> f64 {
        core::f64::consts::PI * self.radius * self.radius
    }
}

impl Shape for Square {
    type Unit = f64;

    fn name(&self) -> &'static str {
        "square"
    }

    fn perimeter(&self) -> f64 {
        4.0 * self.side
    }
}

impl Shape for Circle {
    type Unit = f64;

    fn name(&self) -> &'static str {
        "circle"
    }
}

/// An associated const, kept apart from `Shape` so that `dyn Shape` is allowed.
pub trait Sides {
    const SIDES: u32;
}

impl Sides for Square {
    const SIDES: u32 = 4;
}

/// A blanket impl over references.
impl<T: Area + ?Sized> Area for &T {
    fn area(&self) -> f64 {
        (**self).area()
    }
}

/// A blanket impl over boxes of trait objects.
impl Area for Box<dyn Shape<Unit = f64>> {
    fn area(&self) -> f64 {
        (**self).area()
    }
}

/// Upcasting a trait object to its supertrait.
pub fn as_area(shape: &dyn Shape<Unit = f64>) -> &dyn Area {
    shape
}

pub fn total_area<'a, I>(shapes: I) -> f64
where
    I: IntoIterator<Item = &'a dyn Shape<Unit = f64>>,
{
    shapes.into_iter().map(|s| s.area()).sum()
}

/// A 2D vector with operators.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct V2 {
    pub x: f64,
    pub y: f64,
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2 { x: self.x + o.x, y: self.y + o.y }
    }
}

impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, k: f64) -> V2 {
        V2 { x: self.x * k, y: self.y * k }
    }
}

impl Index<usize> for V2 {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        match i {
            0 => &self.x,
            1 => &self.y,
            _ => panic!("V2 has two components"),
        }
    }
}

impl fmt::Display for V2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:.1}, {:.1})", self.x, self.y)
    }
}

impl From<(f64, f64)> for V2 {
    fn from((x, y): (f64, f64)) -> V2 {
        V2 { x, y }
    }
}

/// A trait with a generic method and a generic associated type.
pub trait Container {
    type Item<'a>
    where
        Self: 'a;
    type Iter<'a>: Iterator<Item = Self::Item<'a>>
    where
        Self: 'a;

    fn items(&self) -> Self::Iter<'_>;

    fn first_matching<P: FnMut(&Self::Item<'_>) -> bool>(&self, mut p: P) -> Option<Self::Item<'_>> {
        self.items().find(|i| p(i))
    }
}

impl<T> Container for Vec<T> {
    type Item<'a>
        = &'a T
    where
        T: 'a;
    type Iter<'a>
        = core::slice::Iter<'a, T>
    where
        T: 'a;

    fn items(&self) -> Self::Iter<'_> {
        self.iter()
    }
}

/// Return-position `impl Trait` in a trait.
pub trait Labels {
    fn labels(&self) -> impl Iterator<Item = String> + '_;
}

impl Labels for [Square] {
    fn labels(&self) -> impl Iterator<Item = String> + '_ {
        self.iter().map(|s| format!("square {}", s.side))
    }
}

/// Higher-ranked trait bounds.
pub fn apply_to_all<F>(items: &[String], f: F) -> Vec<usize>
where
    F: for<'a> Fn(&'a str) -> &'a str,
{
    items.iter().map(|s| f(s).len()).collect()
}

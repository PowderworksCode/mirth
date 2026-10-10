//! A crate for mirth-lab audit-options: generic code, code of its own for codegen
//! options to change, a closure, a static, and warnings for diagnostic options.

use std::collections::BTreeMap;
use std::fmt::Display;

pub trait Shape {
    fn area(&self) -> f64;
    fn name(&self) -> String {
        format!("shape of area {:.2}", self.area())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Square(pub f64);

impl Shape for Square {
    fn area(&self) -> f64 {
        self.0 * self.0
    }
}

pub fn largest<T: Shape>(shapes: &[T]) -> Option<&T> {
    shapes.iter().max_by(|a, b| a.area().total_cmp(&b.area()))
}

pub fn describe<T: Display>(items: &[T]) -> String {
    items.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ")
}

/// Code of its own, with a stack frame worth measuring.
pub fn checksum(values: &[u64]) -> u64 {
    let mut buffer = [0u64; 64];
    for (i, v) in values.iter().enumerate() {
        buffer[i % 64] = buffer[i % 64].wrapping_mul(31).wrapping_add(*v);
    }
    buffer.iter().fold(0, |acc, x| acc ^ x.rotate_left(7))
}

pub fn histogram(words: &str) -> BTreeMap<usize, usize> {
    let mut counts = BTreeMap::new();
    for w in words.split_whitespace() {
        *counts.entry(w.len()).or_insert(0) += 1;
    }
    counts
}

pub fn multiplier(n: u32) -> impl Fn(u32) -> u32 {
    move |x| x.wrapping_mul(n)
}

pub static TABLE: [u32; 4] = [1, 2, 3, 4];

fn unused_helper() -> u32 {
    let unused_value = 3;
    4
}

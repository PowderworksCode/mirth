//! A module found through `#[path]`.

pub fn twice(x: u32) -> u32 {
    x * 2
}

pub use super::lookup as find;

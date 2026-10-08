//! The bottom of the sink: as many stable language features as fit, for
//! dependents to read through metadata.
#![warn(missing_debug_implementations)]
#![allow(dead_code)]

extern crate alloc;

pub mod algo;
pub mod asyncs;
pub mod consts;
pub mod errors;
pub mod extras;
pub mod memory;
pub mod shapes;
#[macro_use]
mod macros;

pub use shapes::{Area, Circle, Shape, Square};
pub use extras::inner::*;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

use core::sync::atomic::AtomicUsize;

/// Counted by `#[sink_macros::traced]`.
pub static TRACE: AtomicUsize = AtomicUsize::new(0);

/// Implemented by `#[derive(sink_macros::Describe)]`.
pub trait Describe {
    const NAME: &'static str;
    const FIELDS: usize;

    fn tag(&self) -> &'static str;

    fn describe(&self) -> String {
        format!("{} with {} fields, tagged {}", Self::NAME, Self::FIELDS, self.tag())
    }
}

#[cfg(sink_generated)]
pub fn generated() -> bool {
    true
}

#[cfg(feature = "extra")]
pub fn extra() -> &'static str {
    "extra"
}

/// A doc link to [`Shape`], [`algo::Pipeline`] and [`crate::consts::Matrix`].
pub fn version() -> (u32, &'static str) {
    (PRIMES[3], GENERATED_NAME)
}

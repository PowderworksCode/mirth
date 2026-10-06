//! A rustc driver that Cargo accepts as a wrapper, and that hands each
//! function's MIR to a plugin, which can replace it.
//!
//! ```ignore
//! fn main() -> ! {
//!     mirth::run(env!("MIRTH_SYSROOT"), MyPlugin)
//! }
//! ```
//!
//! with `mirth_build::link_to_the_toolchain()` in the plugin's `build.rs`.
//! `rustc_private` has no stability guarantee, so this is built against the
//! nightly in `rust-toolchain.toml`.

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

pub mod config;
pub mod emit;
pub mod identity;
pub mod plugin;

pub use plugin::{Plugin, run};

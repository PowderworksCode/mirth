//! Change a Rust program's MIR from a rustc driver that Cargo accepts.
//!
//! A plugin implements [`Plugin`] and calls [`run`]. Mirth does the rest:
//! being a `rustc` that Cargo will run as a wrapper, overriding
//! `optimized_mir` so the plugin sees each body and can hand back another,
//! injecting a crate the program never named, and writing MIR by hand.
//!
//! ```ignore
//! # struct Counting;
//! # impl mirth::Plugin for Counting {}
//! fn main() -> ! {
//!     mirth::run(env!("MIRTH_SYSROOT"), Counting)
//! }
//! ```
//!
//! with a `build.rs` of one line, [`mirth_build::link_to_the_toolchain`].
//!
//! # The compiler this is written against
//!
//! `rustc_private` has no stability guarantee: an API that exists on one
//! nightly can be gone on the next. `rust-toolchain.toml` pins the one this
//! compiles against, and HACKING.md records every workaround.

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

//! What Cargo tells a plugin about the crate it is compiling.
//!
//! A plugin is a `rustc` wrapper, so Cargo runs it once per crate with an
//! environment that answers three questions the plugin would otherwise have to
//! guess at. All three are verified against the pinned toolchain rather than
//! assumed; a wrapper on a package with one path dependency sees:
//!
//! ```text
//!   crate=probe primary=1   manifest=…/probe/Cargo.toml
//!   crate=dep   primary=no  manifest=…/probe/dep/Cargo.toml
//!   crate=?     primary=no  manifest=none        <- Cargo probing the compiler
//! ```

use std::path::PathBuf;

/// Whether this crate is one the user asked Cargo to build.
///
/// Handed over by Cargo rather than approximated by choosing between `rustc-wrapper` and `rustc-workspace-wrapper`. Those two decide
/// which crates the plugin is *spawned* for, which is a question about process
/// count; this decides what it does once it is running.
///
/// The sound default is to touch the primary package and to leave everything
/// else alone. A library can still opt itself in through its own manifest.
/// Nothing opts a stranger's code in by being depended on.
fn primary() -> bool {
    std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some()
}

/// Whether this crate is somebody else's, being built on the way to the
/// user's.
///
/// The question a plugin usually wants, and not quite the negation of
/// [`primary`]: a plugin invoked by hand, with no Cargo anywhere, is not
/// compiling a dependency. It is compiling exactly what it was pointed at.
pub fn a_dependency() -> bool {
    manifest().is_some() && !primary()
}

/// The manifest of the crate being compiled, if Cargo is the one asking.
///
/// Absent when the plugin is invoked directly, and absent for the probe
/// invocation Cargo makes before it uses a compiler at all.
pub fn manifest() -> Option<PathBuf> {
    std::env::var_os("CARGO_MANIFEST_PATH")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

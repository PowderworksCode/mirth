//! What Cargo tells a wrapper about the crate it is compiling.
//!
//! Cargo runs a wrapper once per crate. For a package with one path
//! dependency it sets:
//!
//! ```text
//!   crate   CARGO_PRIMARY_PACKAGE   CARGO_MANIFEST_PATH
//!   probe   1                       …/probe/Cargo.toml
//!   dep     (unset)                 …/probe/dep/Cargo.toml
//!   -       (unset)                 (unset)              Cargo asking the compiler its version
//! ```

use std::path::PathBuf;

/// Whether Cargo was asked to build this crate's package.
fn primary() -> bool {
    std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some()
}

/// Whether this crate is a dependency of what Cargo was asked to build. False
/// when the plugin is run without Cargo.
pub fn a_dependency() -> bool {
    manifest().is_some() && !primary()
}

/// The manifest of the crate being compiled, when Cargo is running the
/// plugin.
pub fn manifest() -> Option<PathBuf> {
    std::env::var_os("CARGO_MANIFEST_PATH")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

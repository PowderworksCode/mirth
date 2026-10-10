//! Checks (oracles) over rustc: the shared parts. Each check is a subcommand of the
//! `mirth-lab` binary, in `src/tools/`; docs/checks.md says what each looks for and found.

pub mod artifacts;
pub mod coverage;
pub mod driver;
pub mod miri;
pub mod normalize;
pub mod rustc;
pub mod uitest;

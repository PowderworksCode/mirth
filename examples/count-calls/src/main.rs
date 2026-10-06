//! The smallest mirth plugin: counts the calls in the crates Cargo was asked
//! to build, and changes nothing.
//!
//! ```text
//! $ RUSTC_WRAPPER=count-calls cargo build      # in fixtures/two-crates
//! count-calls: `app`: 5 calls in 1 function
//! ```

#![feature(rustc_private)]

extern crate rustc_middle;
extern crate rustc_span;

use rustc_middle::mir::{Body, TerminatorKind};
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::{LOCAL_CRATE, LocalDefId};

#[derive(Default)]
struct CountCalls {
    calls: usize,
    functions: usize,
}

impl mirth::Plugin for CountCalls {
    fn body<'tcx>(
        &mut self,
        _tcx: TyCtxt<'tcx>,
        _def_id: LocalDefId,
        body: &'tcx Body<'tcx>,
    ) -> Option<&'tcx Body<'tcx>> {
        self.functions += 1;
        self.calls += body
            .basic_blocks
            .iter()
            .filter(|block| matches!(block.terminator().kind, TerminatorKind::Call { .. }))
            .count();
        None
    }

    fn finished(&mut self, tcx: TyCtxt<'_>) {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        eprintln!(
            "count-calls: `{}`: {} in {}",
            tcx.crate_name(LOCAL_CRATE),
            plural(self.calls, "call", "calls"),
            plural(self.functions, "function", "functions"),
        );
    }
}

/// For dependencies: compile as plain rustc would.
struct Untouched;
impl mirth::Plugin for Untouched {}

fn main() -> ! {
    if mirth::config::a_dependency() {
        mirth::run(env!("MIRTH_SYSROOT"), Untouched)
    }
    mirth::run(env!("MIRTH_SYSROOT"), CountCalls::default())
}

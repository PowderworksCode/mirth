//! Writing MIR out by hand.
//!
//! Everything a plugin adds to a body needs the same span and the same source
//! info, and two calls that differ only in their callee and their arguments
//! spell out to a paragraph each, in which the differences are the hard part
//! to see. [`Build`] is that paragraph, once.
//!
//! Nothing here checks anything. `optimized_mir` runs long after the passes
//! that would have objected, so MIR put in through this is never asked whether
//! it was allowed — not by the borrow checker, not by the unsafety checker,
//! not by anything. A plugin that emits a place of the wrong type finds out
//! from an ICE deep inside codegen, or does not find out at all. See
//! HACKING.md.

use rustc_middle::mir::{
    BasicBlock, CallSource, Const, ConstOperand, ConstValue, Local, Operand, Place, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind, UnwindAction, WithRetag,
};
use rustc_middle::ty::{Ty, TyCtxt};
use rustc_span::def_id::DefId;
use rustc_span::{Span, Spanned};

/// Somewhere to put the span, the source info and the constants a plugin
/// keeps handing to the same calls.
pub struct Build<'tcx> {
    tcx: TyCtxt<'tcx>,
    pub span: Span,
    source_info: SourceInfo,
}

impl<'tcx> Build<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, span: Span) -> Build<'tcx> {
        Build {
            tcx,
            span,
            source_info: SourceInfo::outermost(span),
        }
    }

    pub fn copy(&self, place: impl Into<Place<'tcx>>) -> Operand<'tcx> {
        Operand::Copy(place.into())
    }

    /// A `u64` constant.
    pub fn number(&self, value: u64) -> Operand<'tcx> {
        self.literal(
            ConstValue::Scalar(rustc_middle::mir::interpret::Scalar::from_u64(value)),
            self.tcx.types.u64,
        )
    }

    fn literal(&self, value: ConstValue, ty: Ty<'tcx>) -> Operand<'tcx> {
        Operand::Constant(Box::new(ConstOperand {
            span: self.span,
            user_ty: None,
            const_: Const::Val(value, ty),
        }))
    }

    pub fn assign(&self, place: impl Into<Place<'tcx>>, from: Operand<'tcx>) -> Statement<'tcx> {
        Statement::new(
            self.source_info,
            StatementKind::Assign(Box::new((place.into(), Rvalue::Use(from, WithRetag::No)))),
        )
    }

    /// A call to `callee`, with `over` as its generic arguments.
    ///
    /// A function item is zero-sized, its identity entirely in its type, so
    /// the callee is a `ZeroSized` constant whose type names it. Collection
    /// picks the instance up from this MIR the same way it would from a call
    /// the programmer wrote. Unwinding continues to the caller's caller: the
    /// call holds nothing to clean up.
    pub fn call_over(
        &self,
        callee: DefId,
        over: &[Ty<'tcx>],
        arguments: impl IntoIterator<Item = Operand<'tcx>>,
        destination: Local,
        target: BasicBlock,
    ) -> Terminator<'tcx> {
        let generics: Vec<rustc_middle::ty::GenericArg<'tcx>> =
            over.iter().copied().map(Into::into).collect();
        self.called(callee, generics, arguments, destination, target)
    }

    fn called(
        &self,
        callee: DefId,
        generics: Vec<rustc_middle::ty::GenericArg<'tcx>>,
        arguments: impl IntoIterator<Item = Operand<'tcx>>,
        destination: Local,
        target: BasicBlock,
    ) -> Terminator<'tcx> {
        let ty = Ty::new_fn_def(self.tcx, callee, rustc_middle::ty::Binder::dummy(generics));
        self.terminator(TerminatorKind::Call {
            func: self.literal(ConstValue::ZeroSized, ty),
            args: arguments
                .into_iter()
                .map(|node| Spanned {
                    node,
                    span: self.span,
                })
                .collect(),
            destination: Place::from(destination),
            target: Some(target),
            unwind: UnwindAction::Continue,
            call_source: CallSource::Misc,
            fn_span: self.span,
        })
    }

    /// `Terminator::attributes` carries things like `#[track_caller]` for a
    /// call. Mirth's have none, and building them through here means the next
    /// field this struct grows is one edit rather than five.
    pub fn terminator(&self, kind: TerminatorKind<'tcx>) -> Terminator<'tcx> {
        Terminator {
            source_info: self.source_info,
            kind,
            attributes: Default::default(),
        }
    }
}

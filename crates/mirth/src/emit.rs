//! Building MIR statements and terminators by hand.
//!
//! Nothing checks what is built here. `optimized_mir` runs after borrow
//! checking and the other MIR checks, so a wrong type surfaces as an ICE in
//! code generation, or not at all.

use rustc_middle::mir::{
    BasicBlock, BasicBlockData, Body, BorrowKind, CallSource, CastKind, Const, ConstOperand,
    ConstValue, Local, Location, Operand, Place, Rvalue, SourceInfo, Statement, StatementKind,
    Terminator, TerminatorKind, UnwindAction, WithRetag,
};
use rustc_middle::ty::{Ty, TyCtxt};
use rustc_span::def_id::DefId;
use rustc_span::{Span, Spanned};

/// Builds MIR that all carries one span.
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

    /// `place = &of`, a shared borrow. Regions are erased by the time a body
    /// reaches `optimized_mir`, so the borrow carries the erased one.
    pub fn reference(&self, place: impl Into<Place<'tcx>>, of: Place<'tcx>) -> Statement<'tcx> {
        Statement::new(
            self.source_info,
            StatementKind::Assign(Box::new((
                place.into(),
                Rvalue::Ref(self.tcx.lifetimes.re_erased, BorrowKind::Shared, of),
            ))),
        )
    }

    /// `place = transmute::<_, ty>(from)`.
    pub fn transmute(
        &self,
        place: impl Into<Place<'tcx>>,
        from: Operand<'tcx>,
        ty: Ty<'tcx>,
    ) -> Statement<'tcx> {
        Statement::new(
            self.source_info,
            StatementKind::Assign(Box::new((
                place.into(),
                Rvalue::Cast(CastKind::Transmute, from, ty),
            ))),
        )
    }

    pub fn assign(&self, place: impl Into<Place<'tcx>>, from: Operand<'tcx>) -> Statement<'tcx> {
        Statement::new(
            self.source_info,
            StatementKind::Assign(Box::new((place.into(), Rvalue::Use(from, WithRetag::No)))),
        )
    }

    /// A call to `callee` with `over` as its generic arguments, continuing at
    /// `target`. A panic in the callee unwinds to this function's caller.
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

    pub fn terminator(&self, kind: TerminatorKind<'tcx>) -> Terminator<'tcx> {
        Terminator {
            source_info: self.source_info,
            kind,
            loop_hint_attrs: Default::default(),
        }
    }
}

/// Split a block before `at`, and return the new block holding everything
/// from `at` on, the original terminator included.
///
/// The original block is left without a terminator, for the caller to end
/// with whatever it inserts. `at.statement_index` may equal the number of
/// statements, which splits just before the terminator.
pub fn split<'tcx>(body: &mut Body<'tcx>, at: Location) -> BasicBlock {
    let blocks = body.basic_blocks_mut();
    let head = &mut blocks[at.block];
    let rest = head.statements.split_off(at.statement_index);
    let terminator = head.terminator.take();
    let is_cleanup = head.is_cleanup;
    blocks.push(BasicBlockData::new_stmts(rest, terminator, is_cleanup))
}

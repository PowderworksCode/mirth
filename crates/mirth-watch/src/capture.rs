//! How an argument is written down, decided from its type while compiling.
//!
//! MIR handed back through `optimized_mir` is never type-checked again, so a
//! hook is only ever called with a type it is known to accept:
//!
//! - **text and numbers** (`str`, `String`, `Path`, `PathBuf`, `OsStr`,
//!   `OsString`, integers, `bool`, `char`), through the runtime's `Capture`;
//! - **plain data**: a struct or tuple whose fields are, recursively, numbers
//!   (including pattern types over integers, which rustc's index types use),
//!   such as a `DefId`. Each number is captured and the runtime joins them, as
//!   `2:15`. Rendering it any other way could run the program's own code;
//! - **`Debug`**, only where the configuration asks for it, and only for a
//!   type that implements it. That does run the program's code, which may
//!   have effects of its own, so it is never the default.

use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::mir::{PlaceElem, ProjectionElem};
use rustc_middle::ty::{self, Ty, TyCtxt, TypeVisitableExt, TypingEnv};
use rustc_span::sym;
use rustc_trait_selection::infer::InferCtxtExt;

pub enum How<'tcx> {
    Text(Ty<'tcx>),
    /// Each number in the value: where it is, the type to capture it as, and
    /// its own type, which differs for a pattern type such as
    /// `u32 is 0..=0xFFFF_FF00` and is then transmuted to the first.
    Plain(Vec<(Vec<PlaceElem<'tcx>>, Ty<'tcx>, Ty<'tcx>)>),
    Debug(Ty<'tcx>),
}

const MOST_FIELDS: usize = 8;
const DEEPEST: usize = 4;

pub fn how<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, debug: bool) -> Option<How<'tcx>> {
    if ty.has_param() || ty.has_aliases() {
        return None;
    }
    if debug {
        return implements_debug(tcx, ty).then_some(How::Debug(ty));
    }
    if text(tcx, ty) {
        return Some(How::Text(ty));
    }
    let mut leaves = Vec::new();
    plain(tcx, ty, Vec::new(), 0, &mut leaves)?;
    (!leaves.is_empty()).then_some(How::Plain(leaves))
}

fn text<'tcx>(tcx: TyCtxt<'tcx>, mut ty: Ty<'tcx>) -> bool {
    while let ty::Ref(_, inner, _) = ty.kind() {
        ty = *inner;
    }
    match ty.kind() {
        ty::Str | ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) => true,
        ty::Adt(adt, _) => matches!(
            crate::sites::path_of(tcx, adt.did()).as_str(),
            "std::string::String"
                | "std::path::Path"
                | "std::path::PathBuf"
                | "std::ffi::OsStr"
                | "std::ffi::OsString"
                | "alloc::string::String"
                | "std::ffi::os_str::OsStr"
                | "std::ffi::os_str::OsString"
        ),
        _ => false,
    }
}

fn plain<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    at: Vec<PlaceElem<'tcx>>,
    depth: usize,
    leaves: &mut Vec<(Vec<PlaceElem<'tcx>>, Ty<'tcx>, Ty<'tcx>)>,
) -> Option<()> {
    if depth > DEEPEST || leaves.len() > MOST_FIELDS {
        return None;
    }
    match ty.kind() {
        ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) => {
            leaves.push((at, ty, ty));
            Some(())
        }
        ty::Pat(base, _) if matches!(base.kind(), ty::Int(_) | ty::Uint(_)) => {
            leaves.push((at, *base, ty));
            Some(())
        }
        ty::Tuple(fields) => {
            for (index, field_ty) in fields.iter().enumerate() {
                let mut deeper = at.clone();
                deeper.push(ProjectionElem::Field(index.into(), field_ty));
                plain(tcx, field_ty, deeper, depth + 1, leaves)?;
            }
            Some(())
        }
        ty::Ref(_, inner, _) => {
            let mut deeper = at;
            deeper.push(ProjectionElem::Deref);
            plain(tcx, *inner, deeper, depth + 1, leaves)
        }
        ty::Adt(adt, args) if adt.is_struct() => {
            for (index, field) in adt.non_enum_variant().fields.iter_enumerated() {
                let field_ty = field.ty(tcx, args).skip_normalization();
                let mut deeper = at.clone();
                deeper.push(ProjectionElem::Field(index, field_ty));
                plain(tcx, field_ty, deeper, depth + 1, leaves)?;
            }
            Some(())
        }
        _ => None,
    }
}

fn implements_debug<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let Some(debug) = tcx.get_diagnostic_item(sym::Debug) else {
        return false;
    };
    let (infcx, param_env) = tcx
        .infer_ctxt()
        .build_with_typing_env(TypingEnv::fully_monomorphized());
    infcx
        .type_implements_trait(debug, [ty], param_env)
        .must_apply_modulo_regions()
}

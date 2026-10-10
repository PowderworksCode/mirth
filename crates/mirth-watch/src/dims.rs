//! The coverage dimensions beyond blocks (docs/coverage-plan.md): branch arms and the
//! configuration they depend on, and keys that tell a function's runs apart.

use rustc_abi::FieldIdx;
use rustc_middle::mir::{
    BasicBlock, Body, CastKind, Local, Operand, Place, PlaceElem, ProjectionElem, Rvalue,
    Statement, StatementKind, TerminatorKind,
};
use rustc_middle::ty::{self, AdtDef, Ty, TyCtxt};

use crate::sites::{defining_path_of, path_of};

/// Where a key comes from: a place read at entry (an argument, through fields), or the return
/// place at each return; read as an integer, or as an enum's discriminant.
pub struct Key<'tcx> {
    pub place: Place<'tcx>,
    pub ty: Ty<'tcx>,
    pub at_return: bool,
    /// The enum whose discriminant is the key, when it is one.
    pub discriminant: Option<AdtDef<'tcx>>,
    pub label: String,
}

/// Resolve a key specification against a body's arguments: `None` when the body has no such
/// argument or field, or the value at the end is not something with an integer reading.
pub fn resolve_key<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, spec: &str, label: &str) -> Option<Key<'tcx>> {
    let (mut steps, discriminant_wanted) = match spec.strip_suffix('#') {
        Some(rest) => (rest.split('.'), true),
        None => (spec.split('.'), false),
    };
    let source = steps.next()?;
    let (local, at_return) = if source == "ret" {
        (Local::from_usize(0), true)
    } else if let Some(n) = source.strip_prefix("arg") {
        let n: usize = n.parse().ok()?;
        if n >= body.arg_count {
            return None;
        }
        (Local::from_usize(n + 1), false)
    } else if let Some(suffix) = source.strip_prefix("type:") {
        let found = (1..=body.arg_count).map(Local::from_usize).find(|&local| {
            let ty = peel(body.local_decls[local].ty);
            match ty.kind() {
                ty::Adt(adt, _) => {
                    let path = defining_path_of(tcx, adt.did());
                    path == suffix || path.ends_with(&format!("::{suffix}"))
                }
                _ => false,
            }
        })?;
        (found, false)
    } else {
        return None;
    };
    let mut ty = body.local_decls[local].ty;
    let mut projection: Vec<PlaceElem<'tcx>> = Vec::new();
    for step in steps {
        while let ty::Ref(_, pointee, _) = ty.kind() {
            projection.push(ProjectionElem::Deref);
            ty = *pointee;
        }
        let (index, field_ty) = field(tcx, ty, step)?;
        projection.push(ProjectionElem::Field(index, field_ty));
        ty = field_ty;
    }
    loop {
        while let ty::Ref(_, pointee, _) = ty.kind() {
            projection.push(ProjectionElem::Deref);
            ty = *pointee;
        }
        match ty.kind() {
            ty::Adt(adt, _) if adt.is_enum() => break,
            ty::Adt(adt, args) if adt.is_struct() && !discriminant_wanted => {
                // Look through a one-field struct (a newtype index, a `DepKind`-like wrapper).
                let variant = adt.non_enum_variant();
                if variant.fields.len() != 1 {
                    return None;
                }
                let index = FieldIdx::from_usize(0);
                let field_ty = variant.fields[index].ty(tcx, args).skip_normalization();
                projection.push(ProjectionElem::Field(index, field_ty));
                ty = field_ty;
            }
            ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) if !discriminant_wanted => break,
            _ => return None,
        }
    }
    let discriminant = match ty.kind() {
        ty::Adt(adt, _) if adt.is_enum() => Some(*adt),
        _ => None,
    };
    Some(Key {
        place: Place { local, projection: tcx.mk_place_elems(&projection) },
        ty,
        at_return,
        discriminant,
        label: label.to_owned(),
    })
}

fn peel(mut ty: Ty<'_>) -> Ty<'_> {
    while let ty::Ref(_, pointee, _) = ty.kind() {
        ty = *pointee;
    }
    ty
}

/// A struct's or tuple's field by name or number.
fn field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, step: &str) -> Option<(FieldIdx, Ty<'tcx>)> {
    match ty.kind() {
        ty::Adt(adt, args) if adt.is_struct() => {
            let variant = adt.non_enum_variant();
            let index = match step.parse::<usize>() {
                Ok(n) if n < variant.fields.len() => FieldIdx::from_usize(n),
                Ok(_) => return None,
                Err(_) => variant.fields.iter_enumerated().find(|(_, f)| f.name.as_str() == step)?.0,
            };
            Some((index, variant.fields[index].ty(tcx, args).skip_normalization()))
        }
        ty::Tuple(types) => {
            let n: usize = step.parse().ok()?;
            Some((FieldIdx::from_usize(n), *types.get(n)?))
        }
        _ => None,
    }
}

/// The statements that read `key` into `into`, a fresh `u64` local (`scratch` gets a local for a
/// discriminant first, when the key is one).
pub fn read_key<'tcx>(
    tcx: TyCtxt<'tcx>,
    build: &mirth::emit::Build<'tcx>,
    key: &Key<'tcx>,
    into: Local,
    scratch: impl FnOnce(Ty<'tcx>) -> Local,
) -> Vec<Statement<'tcx>> {
    let source_info = rustc_middle::mir::SourceInfo::outermost(build.span);
    let assign = |place: Place<'tcx>, rvalue: Rvalue<'tcx>| Statement::new(source_info, StatementKind::Assign(Box::new((place, rvalue))));
    let u64_ty = tcx.types.u64;
    if key.discriminant.is_some() {
        let discriminant_ty = key.ty.discriminant_ty(tcx);
        let d = scratch(discriminant_ty);
        vec![
            assign(Place::from(d), Rvalue::Discriminant(key.place)),
            assign(Place::from(into), Rvalue::Cast(CastKind::IntToInt, Operand::Copy(Place::from(d)), u64_ty)),
        ]
    } else {
        vec![assign(Place::from(into), Rvalue::Cast(CastKind::IntToInt, Operand::Copy(key.place), u64_ty))]
    }
}

/// An enum's variants with their discriminant values, for the site table's key names.
pub fn variants<'tcx>(tcx: TyCtxt<'tcx>, adt: AdtDef<'tcx>) -> Vec<(u128, String)> {
    adt.discriminants(tcx).map(|(index, discr)| (discr.val, adt.variant(index).name.to_string())).collect()
}

/// One arm of a switch whose target other paths reach too.
pub struct Arm {
    pub switch: BasicBlock,
    pub index: usize,
    pub target: BasicBlock,
    /// The configuration the switch reads: `feature:<name>`, `option:<name>`, `edition`,
    /// `target:<field>`.
    pub config: Option<String>,
    /// Whether other paths reach the target too. An arm into a target only it reaches needs no
    /// site of its own (the target's block site says it ran); such arms are listed only for
    /// switches that read configuration.
    pub shared: bool,
}

/// The arms of every switch outside cleanup whose target has more than one predecessor (an arm
/// into a target only it reaches is told by the target's own block site), and every arm of a
/// switch that reads configuration, leaving out targets that are empty `unreachable` blocks.
pub fn arms<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Vec<Arm> {
    let predecessors = body.basic_blocks.predecessors();
    let definitions = Definitions::of(body);
    let mut out = Vec::new();
    for (switch, data) in body.basic_blocks.iter_enumerated() {
        if data.is_cleanup {
            continue;
        }
        let TerminatorKind::SwitchInt { discr, targets } = &data.terminator().kind else { continue };
        let config = configuration(tcx, body, &definitions, discr, 0);
        for (index, &target) in targets.all_targets().iter().enumerate() {
            let target_data = &body.basic_blocks[target];
            let never = target_data.statements.is_empty() && matches!(target_data.terminator().kind, TerminatorKind::Unreachable);
            let shared = predecessors[target].len() > 1;
            if never || target_data.is_cleanup || (!shared && config.is_none()) {
                continue;
            }
            out.push(Arm { switch, index, target, config: config.clone(), shared });
        }
    }
    out
}

/// Where each local is assigned, when it is assigned exactly once without a projection.
struct Definitions<'a, 'tcx> {
    by_local: Vec<Vec<Definition<'a, 'tcx>>>,
}

enum Definition<'a, 'tcx> {
    Statement(&'a Rvalue<'tcx>),
    Call(&'a Operand<'tcx>),
}

impl<'a, 'tcx> Definitions<'a, 'tcx> {
    fn of(body: &'a Body<'tcx>) -> Self {
        let mut by_local: Vec<Vec<Definition<'a, 'tcx>>> = (0..body.local_decls.len()).map(|_| Vec::new()).collect();
        for data in body.basic_blocks.iter() {
            for statement in &data.statements {
                if let StatementKind::Assign(assign) = &statement.kind
                    && assign.0.projection.is_empty()
                {
                    by_local[assign.0.local.as_usize()].push(Definition::Statement(&assign.1));
                }
            }
            if let TerminatorKind::Call { func, destination, .. } = &data.terminator().kind
                && destination.projection.is_empty()
            {
                by_local[destination.local.as_usize()].push(Definition::Call(func));
            }
        }
        Definitions { by_local }
    }

    fn only(&self, local: Local) -> Option<&Definition<'a, 'tcx>> {
        match self.by_local.get(local.as_usize())?.as_slice() {
            [one] => Some(one),
            _ => None,
        }
    }
}

/// The configuration an operand's value comes from, following copies, casts, negations and
/// comparisons a few steps back.
fn configuration<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    definitions: &Definitions<'_, 'tcx>,
    operand: &Operand<'tcx>,
    depth: usize,
) -> Option<String> {
    if depth > 4 {
        return None;
    }
    let place = operand.place()?;
    if let Some(found) = target_field(tcx, body, place) {
        return Some(found);
    }
    if !place.projection.is_empty() {
        // A field of a local: what the local came from.
        return local_configuration(tcx, body, definitions, place.local, depth);
    }
    local_configuration(tcx, body, definitions, place.local, depth)
}

fn local_configuration<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    definitions: &Definitions<'_, 'tcx>,
    local: Local,
    depth: usize,
) -> Option<String> {
    match definitions.only(local)? {
        Definition::Call(func) => {
            let (callee, _) = func.const_fn_def()?;
            call_configuration(&defining_path_of(tcx, callee), &path_of(tcx, callee))
        }
        Definition::Statement(rvalue) => match rvalue {
            Rvalue::Use(operand, ..) | Rvalue::Cast(_, operand, _) | Rvalue::UnaryOp(_, operand) => {
                configuration(tcx, body, definitions, operand, depth + 1)
            }
            Rvalue::BinaryOp(_, operands) => configuration(tcx, body, definitions, &operands.0, depth + 1)
                .or_else(|| configuration(tcx, body, definitions, &operands.1, depth + 1)),
            Rvalue::Discriminant(place) => {
                target_field(tcx, body, *place).or_else(|| local_configuration(tcx, body, definitions, place.local, depth + 1))
            }
            _ => None,
        },
    }
}

/// What a call to `path` reads, when it reads configuration.
fn call_configuration(path: &str, visible: &str) -> Option<String> {
    let last = path.rsplit("::").next().unwrap_or(path);
    if path.starts_with("rustc_feature::unstable::Features::") || visible.starts_with("rustc_feature::Features::") {
        return match last {
            "enabled" | "incomplete" | "internal" => Some("feature:?".into()),
            "enabled_features" | "enabled_lang_features" | "enabled_lib_features" | "enabled_features_iter_stable_order" => None,
            name => Some(format!("feature:{name}")),
        };
    }
    if path.starts_with("rustc_session::") && last.starts_with("read_") {
        return Some(format!("option:{}", &last["read_".len()..]));
    }
    if last.starts_with("at_least_rust_") || last == "edition" || last.starts_with("is_rust_20") || last.starts_with("rust_20") {
        return Some("edition".into());
    }
    if path.starts_with("rustc_session::session::Session::") {
        let wanted = ["is_nightly_build", "panic_strategy", "relocation_model", "crt_static", "lto", "opts_debuginfo", "overflow_checks", "ub_checks", "contract_checks", "threads", "target_filesearch", "needs_plt", "emit_lifetime_markers", "generate_proc_macro_decls_symbol"];
        if wanted.contains(&last) {
            return Some(format!("session:{last}"));
        }
    }
    None
}

/// A read through a field of the target specification (`Target` or `TargetOptions`).
fn target_field<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, place: Place<'tcx>) -> Option<String> {
    let mut ty = body.local_decls[place.local].ty;
    for elem in place.projection.iter() {
        match elem {
            ProjectionElem::Field(index, field_ty) => {
                if let ty::Adt(adt, _) = peel(ty).kind()
                    && adt.is_struct()
                {
                    let path = defining_path_of(tcx, adt.did());
                    if path == "rustc_target::spec::Target" || path == "rustc_target::spec::TargetOptions" {
                        return Some(format!("target:{}", adt.non_enum_variant().fields[index].name));
                    }
                }
                ty = field_ty;
            }
            ProjectionElem::Deref => {
                ty = match ty.kind() {
                    ty::Ref(_, pointee, _) | ty::RawPtr(pointee, _) => *pointee,
                    _ => ty.builtin_deref(true).unwrap_or(ty),
                };
            }
            _ => return None,
        }
    }
    None
}

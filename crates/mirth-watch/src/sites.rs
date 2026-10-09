//! Finding the sites in a body, and putting the runtime's calls in front of
//! them.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use rustc_hir::def::DefKind;
use rustc_middle::mir::interpret::{GlobalAlloc, Scalar};
use rustc_middle::mir::visit::Visitor;
use rustc_middle::mir::{
    BasicBlock, Body, Const, ConstOperand, ConstValue, Local, LocalDecl, Location, Operand, Place,
    Rvalue, START_BLOCK, Statement, TerminatorKind,
};
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_middle::ty::{GenericArgs, GenericArgsRef, Ty, TyCtxt, TypingEnv};
use rustc_span::Symbol;
use rustc_span::def_id::{DefId, LocalDefId};

use crate::capture::{self, How};
use crate::config::{Config, Mode};

/// The runtime's functions, found by diagnostic item.
pub struct Hooks {
    enter: DefId,
    exit: DefId,
    argument: DefId,
    argument_debug: DefId,
    join: DefId,
    event: DefId,
    point: DefId,
    cover: Option<DefId>,
}

impl Hooks {
    pub fn find(tcx: TyCtxt<'_>) -> Option<Hooks> {
        let item = |name: &str| tcx.get_diagnostic_item(Symbol::intern(name));
        Some(Hooks {
            enter: item("mirth_enter")?,
            exit: item("mirth_exit")?,
            argument: item("mirth_argument")?,
            argument_debug: item("mirth_argument_debug")?,
            join: item("mirth_join")?,
            event: item("mirth_event")?,
            point: item("mirth_point")?,
            cover: item("mirth_cover"),
        })
    }
}

/// A body's edges for the call graph, as lines of `<crate>.graph`, functions named by their
/// `DefPathHash` (the same from every crate): `body <hash> <path> <trait item hash> <trait item
/// path> <fn, extern or const> <Self type hash> <Self type path>` (`-` for a body implementing no
/// trait item, or not in an impl for a struct or an enum; `extern` for a function with a foreign
/// ABI, `const` for a constant's or a static's initializer), then
/// `edge <caller hash> <callee hash> <kind> <callee path>`, kind
/// `call` (a direct call, the callee as written: a trait item for a call through a trait, plus
/// the closure or function item a call through `Fn*` names), `resolved` (the implementation a
/// trait call resolves to in the caller's context, where it does), `construct` (a struct or an
/// enum this body may build or hold: an aggregate, a constructor, a constant of the type, a type
/// in a local's type or in a call's generic arguments),
/// `ref` (a function or closure used as a value), or `inline` (a callee whose body MIR
/// inlining merged into this one).
pub fn graph<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId, body: &Body<'tcx>, kind: &str) -> Vec<String> {
    use rustc_middle::mir::{AggregateKind, Operand, Rvalue};
    let hash = |id: DefId| tcx.def_path_hash(id).0.to_hex();
    let caller = hash(def_id.to_def_id());
    let implements = if matches!(tcx.def_kind(def_id), DefKind::AssocFn) {
        tcx.associated_item(def_id.to_def_id()).trait_item_def_id()
    } else {
        None
    };
    // Called from outside Rust (C, C++, LLVM callbacks): a root.
    let kind = if kind == "fn"
        && matches!(tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn)
        && !matches!(
            tcx.fn_sig(def_id.to_def_id()).skip_binder().abi(),
            rustc_abi::ExternAbi::Rust | rustc_abi::ExternAbi::RustCall
        ) {
        "extern"
    } else {
        kind
    };
    // The type an impl's method taking `self` is for, when it is a struct or an enum: the
    // analysis counts the method for trait dispatch only once something builds the type.
    let self_adt = tcx
        .impl_of_assoc(def_id.to_def_id())
        .and_then(|imp| match *tcx.type_of(imp).instantiate_identity().skip_normalization().kind() {
            rustc_middle::ty::Adt(adt, _) => Some(adt.did()),
            _ => None,
        });
    // Only a method taking `self` needs a value of its type; `new`, `default`, `decode` make one.
    let self_adt = self_adt.filter(|_| {
        matches!(tcx.def_kind(def_id), DefKind::AssocFn)
            && tcx.associated_item(def_id.to_def_id()).is_method()
    });
    // The trait an impl's function implements, and the struct or enum (outermost) the impl is
    // for: the analysis counts it for trait dispatch only once reachable code needs that type
    // to implement that trait (a call with that bound, a cast to `dyn Trait`).
    let impl_of = implements.and_then(|_| tcx.impl_of_assoc(def_id.to_def_id()));
    let impl_trait = impl_of.and_then(|imp| tcx.impl_opt_trait_id(imp));
    let impl_adt = impl_of.and_then(|imp| {
        match *tcx.type_of(imp).instantiate_identity().skip_normalization().kind() {
            rustc_middle::ty::Adt(adt, _) => Some(adt.did()),
            _ => None,
        }
    });
    let spec = match (impl_of, impl_trait) {
        (Some(imp), Some(t)) => specializes(tcx, t, imp),
        _ => (false, false),
    };
    // Runs only when the compiler has a bug: no path returns, and every path that ends ends in
    // a panic (`bug!`, `unreachable!`, `.unwrap()` on `None`), directly or through such a body.
    let endings = endings(tcx, body);
    let mut out = vec![format!(
        "body\t{caller}\t{}\t{}\t{}\t{kind}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        path_of(tcx, def_id.to_def_id()),
        implements.map_or("-".to_string(), hash),
        implements.map_or("-".to_string(), |it| defining_path_of(tcx, it)),
        self_adt.map_or("-".to_string(), hash),
        self_adt.map_or("-".to_string(), |it| defining_path_of(tcx, it)),
        impl_trait.map_or("-".to_string(), hash),
        impl_trait.map_or("-".to_string(), |it| defining_path_of(tcx, it)),
        impl_adt.map_or("-".to_string(), hash),
        if spec.0 || spec.1 { "specialized" } else { "-" },
        match &endings {
            None => "-",
            Some((true, others)) if others.is_empty() => "ice-only",
            Some(_) => "diverges",
        },
    )];
    // The rest is the analysis's: `diverges <callee>` for each other function ending a path.
    if let Some((_, others)) = &endings {
        for callee in others {
            out.push(format!("diverges\t{caller}\t{}", hash(*callee)));
        }
    }
    // An impl of a specialized trait proves its bounds (`T: SpecIntoSelfProfilingString`) only
    // when monomorphization picks it: their impls are not gated on bounds either.
    // (Only a specializing impl's: the one it specializes has its bounds proved where it is used.)
    if let Some(imp) = impl_of
        && spec.0
    {
        for (clause, _) in tcx.clauses_of(imp).clauses {
            if let Some(t) = clause.as_trait_clause() {
                out.push(format!("specbound\t{}", hash(t.skip_binder().def_id())));
            }
        }
    }
    let mut edges = std::collections::HashSet::new();
    let mut demands = std::collections::HashSet::new();
    struct Refs<'a, 'tcx> {
        tcx: TyCtxt<'tcx>,
        typing_env: TypingEnv<'tcx>,
        edges: &'a mut std::collections::HashSet<(DefId, &'static str)>,
        demands: &'a mut std::collections::HashSet<(DefId, DefId)>,
        locals: &'a rustc_middle::mir::LocalDecls<'tcx>,
        selected: std::collections::HashSet<rustc_middle::ty::TraitRef<'tcx>>,
    }
    impl<'tcx> Refs<'_, 'tcx> {
        /// `trait` needed for every struct and enum in `args`.
        fn demand(&mut self, r#trait: DefId, args: GenericArgsRef<'tcx>) {
            for arg in args.iter().flat_map(|arg| arg.walk()) {
                if let Some(t) = arg.as_type()
                    && let rustc_middle::ty::Adt(adt, _) = *t.kind()
                {
                    self.demands.insert((r#trait, adt.did()));
                }
            }
        }
        /// What using `target` with `args` needs implemented: its bounds (and their
        /// supertraits), and its own trait for its `Self`, if it is a trait's item.
        fn bounds(&mut self, target: DefId, args: GenericArgsRef<'tcx>) {
            if let Some(of) = self.tcx.trait_of_assoc(target) {
                self.demand(of, args);
            }
            if !matches!(
                self.tcx.def_kind(target),
                DefKind::Fn
                    | DefKind::AssocFn
                    | DefKind::Ctor(..)
                    | DefKind::Closure
                    | DefKind::Const { .. }
                    | DefKind::AssocConst { .. }
            ) {
                return;
            }
            let clauses = self.tcx.clauses_of(target).instantiate(self.tcx, args).clauses;
            self.clauses(clauses.into_iter().map(|clause| clause.skip_normalization()).collect(), 0);
        }
        /// The impl that proves `trait_ref` in this body (whose bounds hold for its generic
        /// parameters), with its arguments: what codegen's selection does, also for a generic body.
        fn select(&self, trait_ref: rustc_middle::ty::TraitRef<'tcx>) -> Option<(DefId, GenericArgsRef<'tcx>)> {
            use rustc_infer::infer::TyCtxtInferExt;
            use rustc_middle::ty::TypeVisitableExt;
            let (infcx, param_env) = self.tcx.infer_ctxt().ignoring_regions().build_with_typing_env(self.typing_env);
            let obligation = rustc_infer::traits::Obligation::new(
                self.tcx,
                rustc_infer::traits::ObligationCause::dummy(),
                param_env,
                trait_ref,
            );
            let mut selcx = rustc_trait_selection::traits::SelectionContext::new(&infcx);
            match selcx.select(&obligation) {
                Ok(Some(rustc_middle::traits::ImplSource::UserDefined(imp))) => {
                    let args = infcx.deeply_resolve_ignoring_regions(imp.args);
                    let args = self.tcx.erase_and_anonymize_regions(args);
                    (!args.has_infer()).then_some((imp.impl_def_id, args))
                }
                _ => None,
            }
        }
        /// Each trait bound in `clauses` (with supertraits), normalized (`<Op as TypeOp>::ErrorInfo`
        /// names a type only then); and for one on concrete types, the bounds of the impl that
        /// satisfies it (a blanket impl's `T: From<U>`) and of the trait's associated types
        /// (`type Domain: JoinSemiLattice`), and theirs.
        fn clauses(&mut self, clauses: Vec<rustc_middle::ty::Clause<'tcx>>, depth: usize) {
            use rustc_middle::ty::{TypeVisitableExt, Unnormalized};
            for clause in rustc_infer::traits::util::elaborate(self.tcx, clauses) {
                let Some(t) = clause.as_trait_clause() else { continue };
                let trait_ref = self.tcx.instantiate_bound_regions_with_erased(t).trait_ref;
                self.demand(trait_ref.def_id, trait_ref.args);
                if trait_ref.has_infer() || trait_ref.has_escaping_bound_vars() {
                    continue;
                }
                let trait_ref = self
                    .tcx
                    .try_normalize_erasing_regions(self.typing_env, Unnormalized::new_wip(trait_ref))
                    .unwrap_or(trait_ref);
                self.demand(trait_ref.def_id, trait_ref.args);
                if depth >= 8 || !self.selected.insert(trait_ref) {
                    continue;
                }
                if let Some((impl_def_id, impl_args)) = self.select(trait_ref) {
                    let nested = self.tcx.clauses_of(impl_def_id).instantiate(self.tcx, impl_args).clauses;
                    self.clauses(nested.into_iter().map(|clause| clause.skip_normalization()).collect(), depth + 1);
                }
                let mut bounds = vec![];
                for item in self.tcx.associated_items(trait_ref.def_id).in_definition_order() {
                    if item.is_type() && self.tcx.generics_of(item.def_id).own_params.is_empty() {
                        bounds.extend(
                            self.tcx
                                .explicit_item_bounds(item.def_id)
                                .iter_instantiated_copied(self.tcx, trait_ref.args)
                                .map(|bound| bound.skip_normalization().0),
                        );
                    }
                }
                if !bounds.is_empty() {
                    self.clauses(bounds, depth + 1);
                }
            }
        }
        /// Every struct and enum a type mentions counts as built (an over-approximation).
        fn types_in(&mut self, ty: Ty<'tcx>) {
            for arg in ty.walk() {
                if let Some(t) = arg.as_type()
                    && let rustc_middle::ty::Adt(adt, _) = *t.kind()
                {
                    self.edges.insert((adt.did(), "construct"));
                }
            }
        }
        /// A tuple struct's or variant's constructor, called or used as a function, builds its type.
        fn constructor(&mut self, target: DefId) {
            if let DefKind::Ctor(of, _) = self.tcx.def_kind(target) {
                let parent = self.tcx.parent(target);
                let adt = match of {
                    rustc_hir::def::CtorOf::Struct => parent,
                    rustc_hir::def::CtorOf::Variant => self.tcx.parent(parent),
                };
                self.edges.insert((adt, "construct"));
            }
        }
    }
    impl<'tcx> Visitor<'tcx> for Refs<'_, 'tcx> {
        fn visit_terminator(&mut self, terminator: &rustc_middle::mir::Terminator<'tcx>, at: Location) {
            if let TerminatorKind::Call { func, args, .. } = &terminator.kind {
                if let Some((target, generic_args)) = func.const_fn_def() {
                    self.edges.insert((target, "call"));
                    self.constructor(target);
                    self.bounds(target, generic_args);
                    // Types named in the call's arguments (a unit struct passed by reference
                    // leaves no value behind in optimized MIR).
                    for ty in generic_args.types() {
                        self.types_in(ty);
                    }
                    if self.tcx.trait_of_assoc(target).is_some() {
                        // A closure or function called through `Fn*` is its own body.
                        if let Some(own) = generic_args.types().next().and_then(|ty| match *ty.kind() {
                            rustc_middle::ty::Closure(d, _)
                            | rustc_middle::ty::Coroutine(d, _)
                            | rustc_middle::ty::CoroutineClosure(d, _)
                            | rustc_middle::ty::FnDef(d, _) => Some(d),
                            _ => None,
                        }) {
                            self.edges.insert((own, "call"));
                        }
                        // The implementation, where the caller's types already decide it.
                        if let Ok(Some(instance)) = rustc_middle::ty::Instance::try_resolve(
                            self.tcx,
                            self.typing_env,
                            target,
                            generic_args,
                        ) && let rustc_middle::ty::InstanceKind::Item(resolved) = instance.def
                            && resolved != target
                        {
                            self.edges.insert((resolved, "resolved"));
                            // The impl's own bounds (`impl<A: Step> Iterator for Range<A>`).
                            self.bounds(resolved, instance.args);
                        }
                    }
                }
                // Function items passed as arguments are references, not this call's callee.
                for arg in args.iter() {
                    self.visit_operand(&arg.node, at);
                }
                return;
            }
            self.super_terminator(terminator, at);
        }
        fn visit_operand(&mut self, operand: &Operand<'tcx>, at: Location) {
            if let Some((target, generic_args)) = operand.const_fn_def() {
                self.edges.insert((target, "ref"));
                self.constructor(target);
                self.bounds(target, generic_args);
                // `<T as Debug>::fmt` as a value (formatting arguments): the implementation.
                if self.tcx.trait_of_assoc(target).is_some()
                    && let Ok(Some(instance)) = rustc_middle::ty::Instance::try_resolve(
                        self.tcx,
                        self.typing_env,
                        target,
                        generic_args,
                    )
                    && let rustc_middle::ty::InstanceKind::Item(resolved) = instance.def
                    && resolved != target
                {
                    self.edges.insert((resolved, "resolved"));
                    self.bounds(resolved, instance.args);
                }
            }
            // A constant still to evaluate (`<Combine<X> as AttributeParser>::ATTRIBUTES`): its
            // bounds, and the impl's constant it resolves to.
            if let Operand::Constant(constant) = operand
                && let rustc_middle::mir::Const::Unevaluated(uv, _) = constant.const_
            {
                self.bounds(uv.def, uv.args);
                if self.tcx.trait_of_assoc(uv.def).is_some()
                    && let Ok(Some(instance)) =
                        rustc_middle::ty::Instance::try_resolve(self.tcx, self.typing_env, uv.def, uv.args)
                    && instance.def_id() != uv.def
                {
                    self.edges.insert((instance.def_id(), "resolved"));
                    self.bounds(instance.def_id(), instance.args);
                }
            }
            // A constant of a struct or an enum type (a unit struct, a fieldless variant).
            if let Operand::Constant(constant) = operand
                && let rustc_middle::ty::Adt(adt, _) = *constant.const_.ty().kind()
            {
                self.edges.insert((adt.did(), "construct"));
            }
            self.super_operand(operand, at);
        }
        fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, at: Location) {
            // A cast to `dyn Trait` (a vtable): the source type implements the trait and its
            // supertraits.
            if let Rvalue::Cast(rustc_middle::mir::CastKind::PointerCoercion(
                rustc_middle::ty::adjustment::PointerCoercion::Unsize,
                _,
            ), operand, target) = rvalue
            {
                let source = operand.ty(self.locals, self.tcx);
                for t in target.walk() {
                    if let Some(t) = t.as_type()
                        && let rustc_middle::ty::Dynamic(preds, ..) = *t.kind()
                        && let Some(principal) = preds.principal()
                    {
                        use rustc_middle::ty::Upcast;
                        // The pointee: `&T` and `Box<T>` to `&dyn Trait`, `Box<dyn Trait>`.
                        let pointee = source.builtin_deref(true).unwrap_or(source);
                        let pointee = if pointee.is_box() { pointee.expect_boxed_ty() } else { pointee };
                        for ty in [source, pointee] {
                            let clause: rustc_middle::ty::Clause<'tcx> =
                                principal.with_self_ty(self.tcx, ty).upcast(self.tcx);
                            self.clauses(vec![clause], 0);
                        }
                    }
                }
            }
            if let Rvalue::Aggregate(kind, _) = rvalue {
                if let AggregateKind::Adt(adt, ..) = **kind {
                    self.edges.insert((adt, "construct"));
                }
                match **kind {
                    AggregateKind::Closure(target, _)
                    | AggregateKind::Coroutine(target, _)
                    | AggregateKind::CoroutineClosure(target, _) => {
                        self.edges.insert((target, "ref"));
                    }
                    _ => {}
                }
            }
            self.super_rvalue(rvalue, at);
        }
    }
    let typing_env = TypingEnv::post_analysis(tcx, def_id.to_def_id());
    let mut refs = Refs { tcx, typing_env, edges: &mut edges, demands: &mut demands, locals: &body.local_decls, selected: Default::default() };
    refs.visit_body(body);
    // A body with a local of a type may hold a value of it.
    for local in body.local_decls.iter() {
        refs.types_in(local.ty);
    }
    // A call MIR inlining merged in leaves no call behind: its bounds, with the inlined
    // instance's types.
    for scope in body.source_scopes.iter() {
        if let Some((instance, _)) = scope.inlined {
            refs.bounds(instance.def_id(), instance.args);
        }
    }
    // Promoted constants (`&[f, g]`, `&(f as fn())`) have their own bodies.
    for promoted in tcx.promoted_mir(def_id.to_def_id()).iter() {
        Refs { tcx, typing_env, edges: &mut edges, demands: &mut demands, locals: &promoted.local_decls, selected: Default::default() }
            .visit_body(promoted);
    }
    for scope in body.source_scopes.iter() {
        if let Some((instance, _)) = scope.inlined {
            edges.insert((instance.def_id(), "inline"));
        }
    }
    let mut lines: Vec<String> = edges
        .into_iter()
        .map(|(callee, kind)| format!("edge\t{caller}\t{}\t{kind}\t{}", hash(callee), defining_path_of(tcx, callee)))
        .collect();
    lines.extend(demands.into_iter().map(|(r#trait, adt)| format!("demand\t{caller}\t{}\t{}", hash(r#trait), hash(adt))));
    lines.sort();
    out.extend(lines);
    out
}

/// How a body ends: `None` when some path returns (or yields); otherwise whether it panics on
/// every path, and the other functions that never return which end the rest (a fatal error,
/// `process::exit`, or a helper that panics itself: the analysis decides those).
fn endings<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Option<(bool, Vec<DefId>)> {
    let mut panics = false;
    let mut others = vec![];
    for block in body.basic_blocks.iter() {
        if block.is_cleanup {
            continue;
        }
        match &block.terminator().kind {
            TerminatorKind::Return | TerminatorKind::Yield { .. } | TerminatorKind::CoroutineDrop => {
                return None;
            }
            TerminatorKind::Call { func, target: None, .. } => {
                let Some((callee, _)) = func.const_fn_def() else { return None };
                let panicking = is_panicking(tcx, callee);
                if panicking {
                    panics = true;
                } else {
                    others.push(callee);
                }
            }
            TerminatorKind::TailCall { .. } => return None,
            _ => {}
        }
    }
    Some((panics, others))
}

/// Whether a call to `callee` that does not return is a panic: `panic!`, `bug!`, a failed
/// `unwrap`, an index out of bounds.
fn is_panicking(tcx: TyCtxt<'_>, callee: DefId) -> bool {
    let path = defining_path_of(tcx, callee);
    path.starts_with("core::panicking::")
        || path.starts_with("std::panicking::")
        || path.starts_with("std::rt::begin_panic")
        // `bug!` (through `rustc_span::macros::bug_impl`), its only user here
        || path == "std::panic::panic_any"
        || path.starts_with("core::option::unwrap_failed")
        || path.starts_with("core::option::expect_failed")
        || path.starts_with("core::result::unwrap_failed")
        || path.starts_with("core::slice::index::")
        || path.ends_with("::bug_fmt")
        || path.ends_with("::span_bug_fmt")
        || (path.starts_with("rustc_errors::")
            && (path.ends_with("::bug") || path.ends_with("::span_bug")))
}

/// The blocks every path from which ends in a panic: no path reaches a return, a yield, or a
/// call that diverges for another reason (`FatalError::raise`, `process::exit`), and some path
/// reaches a panicking call. Cleanup blocks lead nowhere here.
fn panic_only_blocks<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
) -> rustc_index::IndexVec<BasicBlock, bool> {
    use rustc_index::IndexVec;
    let blocks = &body.basic_blocks;
    let mut normal: IndexVec<BasicBlock, bool> = IndexVec::from_elem(false, blocks);
    let mut panic: IndexVec<BasicBlock, bool> = IndexVec::from_elem(false, blocks);
    let mut normal_work = vec![];
    let mut panic_work = vec![];
    for (block, data) in blocks.iter_enumerated() {
        if data.is_cleanup {
            continue;
        }
        match &data.terminator().kind {
            TerminatorKind::Return
            | TerminatorKind::Yield { .. }
            | TerminatorKind::CoroutineDrop
            | TerminatorKind::TailCall { .. } => normal_work.push(block),
            TerminatorKind::Call { func, target: None, .. } => {
                if func.const_fn_def().is_some_and(|(callee, _)| is_panicking(tcx, callee)) {
                    panic_work.push(block);
                } else {
                    normal_work.push(block);
                }
            }
            TerminatorKind::InlineAsm { targets, .. } if targets.is_empty() => normal_work.push(block),
            _ => {}
        }
    }
    for (marks, mut work) in [(&mut normal, normal_work), (&mut panic, panic_work)] {
        while let Some(block) = work.pop() {
            if std::mem::replace(&mut marks[block], true) {
                continue;
            }
            for &pred in &blocks.predecessors()[block] {
                if !blocks[pred].is_cleanup && !marks[pred] {
                    work.push(pred);
                }
            }
        }
    }
    normal.into_iter_enumerated().map(|(block, n)| !n && panic[block]).collect()
}

/// Whether `imp` specializes another impl of `r#trait`, or another impl specializes it: which
/// of them a call reaches is then decided at monomorphization, by more than the bounds say.
fn specializes(tcx: TyCtxt<'_>, r#trait: DefId, imp: DefId) -> (bool, bool) {
    let Ok(graph) = tcx.specialization_graph_of(r#trait) else { return (false, false) };
    let specializing = graph.parent.get(&imp).is_some_and(|parent| *parent != r#trait);
    let specialized = graph.children.get(&imp).is_some_and(|children| {
        !children.non_blanket_impls.is_empty() || !children.blanket_impls.is_empty()
    });
    (specializing, specialized)
}

/// A function's defining path, also for one in another crate, where `path_of` prints the
/// path it is visible at (through re-exports): a body's own name in the call graph.
pub fn defining_path_of(tcx: TyCtxt<'_>, def_id: DefId) -> String {
    rustc_middle::ty::print::with_no_visible_paths!(path_of(tcx, def_id))
}

/// The path of every function a body calls directly.
pub fn callees<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Vec<String> {
    body.basic_blocks
        .iter()
        .filter_map(|block| match &block.terminator().kind {
            TerminatorKind::Call { func, .. } => func.const_fn_def(),
            _ => None,
        })
        .map(|(target, _)| path_of(tcx, target))
        .collect()
}

/// One instrumented place in the program, as the logs refer to it.
pub struct Site {
    pub id: u64,
    pub kind: &'static str,
    pub mode: Mode,
    pub caller: String,
    pub target: String,
    pub span: String,
    pub snippet: String,
}

/// A function's path as configurations match it: rustc's own rendering,
/// with the crate's name first.
pub fn path_of(tcx: TyCtxt<'_>, def_id: DefId) -> String {
    let path = with_no_trimmed_paths!(tcx.def_path_str(def_id));
    if def_id.is_local() {
        let path = path.strip_prefix("crate::").unwrap_or(&path);
        format!("{}::{path}", tcx.crate_name(def_id.krate))
    } else {
        path
    }
}

type Captured<'tcx> = Vec<(Operand<'tcx>, How<'tcx>)>;

/// The operands at `capture` and `debug`, each with how it is written down.
/// One that cannot be captured, because the position does not exist or its
/// type is not one the runtime accepts, is left out with a warning naming
/// `site`, so a configuration never records less than it says in silence.
fn captured<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    operands: &[Operand<'tcx>],
    capture: &[usize],
    debug: &[usize],
    site: &str,
) -> Captured<'tcx> {
    let mut indices: Vec<(usize, bool)> = capture
        .iter()
        .map(|&index| (index, false))
        .chain(debug.iter().map(|&index| (index, true)))
        .collect();
    indices.sort();
    indices
        .into_iter()
        .filter_map(|(index, debug)| {
            let Some(operand) = operands.get(index) else {
                eprintln!(
                    "mirth-watch: {site}: there is no argument {index} (it takes {})",
                    operands.len()
                );
                return None;
            };
            let ty = operand.ty(body, tcx);
            let Some(how) = capture::how(tcx, ty, debug) else {
                let instead = if debug {
                    "it does not implement Debug, or is generic here"
                } else {
                    "it is not text, a number or plain data; try `debug`"
                };
                eprintln!(
                    "mirth-watch: {site}: argument {index} is a `{ty}`, not captured: {instead}"
                );
                return None;
            };
            Some((operand.clone(), how))
        })
        .collect()
}

enum What<'tcx> {
    Enter {
        captured: Captured<'tcx>,
    },
    Exit,
    Call {
        target: DefId,
        mode: Mode,
        captured: Captured<'tcx>,
        point: bool,
    },
    Touch {
        target: DefId,
        mode: Mode,
    },
    Cover,
    /// A basic block's start, under `[coverage] blocks`; `panics` when every path from it ends
    /// in a panic (it runs only on a compiler bug).
    Block {
        panics: bool,
    },
}

struct Found<'tcx> {
    at: Location,
    what: What<'tcx>,
}

struct Finder<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    config: &'a Config,
    body: &'a Body<'tcx>,
    found: Vec<Found<'tcx>>,
}

impl<'tcx> Finder<'_, 'tcx> {
    fn in_cleanup(&self, at: Location) -> bool {
        self.body.basic_blocks[at.block].is_cleanup
    }

    fn touch(&mut self, at: Location, target: DefId) {
        let Some(statics) = &self.config.statics else {
            return;
        };
        if self.in_cleanup(at)
            || !is_state(self.tcx, target)
            || !statics.watches(&path_of(self.tcx, target))
        {
            return;
        }
        let mode = statics.mode;
        self.found.push(Found {
            at,
            what: What::Touch { target, mode },
        });
    }
}

/// A static that can change: `static mut`, or one with interior mutability.
/// An immutable `Freeze` static is a constant.
fn is_state(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    if tcx.is_mutable_static(def_id) || tcx.is_thread_local_static(def_id) {
        return true;
    }
    let ty = tcx
        .type_of(def_id)
        .instantiate_identity()
        .skip_normalization();
    !ty.is_freeze(tcx, TypingEnv::fully_monomorphized())
}

impl<'tcx> Visitor<'tcx> for Finder<'_, 'tcx> {
    fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, at: Location) {
        if let Rvalue::ThreadLocalRef(target) = rvalue {
            self.touch(at, *target);
        }
        self.super_rvalue(rvalue, at);
    }

    fn visit_const_operand(&mut self, constant: &ConstOperand<'tcx>, at: Location) {
        if let Const::Val(ConstValue::Scalar(Scalar::Ptr(pointer, _)), _) = constant.const_
            && let GlobalAlloc::Static(target) =
                self.tcx.global_alloc(pointer.provenance.alloc_id())
        {
            self.touch(at, target);
        }
    }

    fn visit_terminator(&mut self, terminator: &rustc_middle::mir::Terminator<'tcx>, at: Location) {
        self.super_terminator(terminator, at);
        if self.in_cleanup(at) {
            return;
        }
        let TerminatorKind::Call { func, args, .. } = &terminator.kind else {
            return;
        };
        let Some((target, _)) = func.const_fn_def() else {
            return;
        };
        let Some(call) = self.config.call(&path_of(self.tcx, target)) else {
            return;
        };
        let operands: Vec<Operand<'tcx>> = args.iter().map(|it| it.node.clone()).collect();
        let site = format!("a call to {}", path_of(self.tcx, target));
        let captured = captured(
            self.tcx,
            self.body,
            &operands,
            &call.capture,
            &call.debug,
            &site,
        );
        self.found.push(Found {
            at,
            what: What::Call {
                target,
                mode: call.mode,
                captured,
                point: call.point,
            },
        });
    }
}

/// A call to put in front of a site.
struct Hook<'tcx> {
    callee: DefId,
    over: Vec<Ty<'tcx>>,
    arguments: Vec<Operand<'tcx>>,
    before: Vec<Statement<'tcx>>,
}

/// The body with the runtime's calls in it, and the sites it now has; or
/// nothing, if no site in it is watched.
pub fn instrument<'tcx>(
    tcx: TyCtxt<'tcx>,
    config: &Config,
    hooks: &Hooks,
    def_id: LocalDefId,
    original: &'tcx Body<'tcx>,
) -> Option<(Body<'tcx>, Vec<Site>)> {
    let mut finder = Finder {
        tcx,
        config,
        body: original,
        found: Vec::new(),
    };
    finder.visit_body(original);
    let mut found = finder.found;

    let caller = path_of(tcx, def_id.to_def_id());
    let is_function = matches!(tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn);
    if is_function && let Some(frame) = config.frame(&caller) {
        let parameters: Vec<Operand<'tcx>> = (1..=original.arg_count)
            .map(|index| Operand::Copy(Place::from(Local::from_usize(index))))
            .collect();
        let site = format!("the frame {caller}");
        let captured = captured(
            tcx,
            original,
            &parameters,
            &frame.capture,
            &frame.debug,
            &site,
        );
        found.push(Found {
            at: START_BLOCK.start_location(),
            what: What::Enter { captured },
        });
        for (block, data) in original.basic_blocks.iter_enumerated() {
            if matches!(data.terminator().kind, TerminatorKind::Return) {
                found.push(Found {
                    at: Location {
                        block,
                        statement_index: data.statements.len(),
                    },
                    what: What::Exit,
                });
            }
        }
    }
    let runs = matches!(tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn | DefKind::Closure);
    if config.coverage.functions && runs && hooks.cover.is_some() {
        found.push(Found { at: START_BLOCK.start_location(), what: What::Cover });
    }
    if config.coverage.blocks && runs && hooks.cover.is_some() {
        let panics = panic_only_blocks(tcx, original);
        for (block, data) in original.basic_blocks.iter_enumerated() {
            // The function's own site stands for its entry block.
            let entry = block == START_BLOCK && config.coverage.functions;
            // Unwinding paths: not instrumented (a call there would need its own unwind edge).
            // An empty `unreachable` block never runs.
            let never = data.statements.is_empty() && matches!(data.terminator().kind, TerminatorKind::Unreachable);
            if entry || data.is_cleanup || never {
                continue;
            }
            found.push(Found { at: block.start_location(), what: What::Block { panics: panics[block] } });
        }
    }
    if found.is_empty() {
        return None;
    }

    let mut body = original.clone();
    let build = mirth::emit::Build::new(tcx, body.span);
    let frame_site = mirth::identity::identity(&format!("{caller}|frame"));
    let mut sites = Vec::new();
    let mut at_location: BTreeMap<Location, Vec<Hook<'tcx>>> = BTreeMap::new();

    for (n, Found { at, what }) in found.into_iter().enumerate() {
        let span = original.source_info(at).span;
        let id = match what {
            What::Enter { .. } | What::Exit => frame_site,
            // The runtime's coverage table keeps sites with the low bit set.
            What::Cover => mirth::identity::identity(&format!("{caller}|cover")) | 1,
            What::Block { .. } => mirth::identity::identity(&format!("{caller}|cover|{:?}", at.block)) | 1,
            _ => mirth::identity::identity(&format!(
                "{caller}|{:?}|{}|{}|{n}",
                at.block,
                at.statement_index,
                tcx.def_path_hash(def_id.to_def_id()).0
            )),
        };
        let mut site = |kind, mode, target: String| {
            sites.push(Site {
                id,
                kind,
                mode,
                caller: caller.clone(),
                target,
                span: describe(tcx, span),
                snippet: snippet(tcx, span),
            })
        };
        let hooks_here = at_location.entry(at).or_default();
        match what {
            What::Enter { captured } => {
                site("frame", Mode::Count, caller.clone());
                hooks_here.extend(argument_hooks(tcx, &build, &mut body, hooks, captured));
                let types: Vec<Ty<'tcx>> = GenericArgs::identity_for_item(tcx, def_id)
                    .types()
                    .collect();
                hooks_here.push(Hook {
                    callee: hooks.enter,
                    over: vec![Ty::new_tup(tcx, &types)],
                    arguments: vec![build.number(id)],
                    before: Vec::new(),
                });
            }
            What::Exit => hooks_here.push(Hook {
                callee: hooks.exit,
                over: Vec::new(),
                arguments: vec![build.number(id)],
                before: Vec::new(),
            }),
            What::Call {
                target,
                mode,
                captured,
                point,
            } => {
                site("call", mode, path_of(tcx, target));
                hooks_here.extend(argument_hooks(tcx, &build, &mut body, hooks, captured));
                hooks_here.push(Hook {
                    callee: hooks.event,
                    over: Vec::new(),
                    arguments: vec![build.number(id), build.number(mode.code())],
                    before: Vec::new(),
                });
                if point {
                    hooks_here.push(Hook {
                        callee: hooks.point,
                        over: Vec::new(),
                        arguments: vec![build.number(id)],
                        before: Vec::new(),
                    });
                }
            }
            What::Block { panics } => {
                let tag = if panics { " panics" } else { "" };
                site("block", Mode::Count, format!("{:?}{tag}", at.block));
                hooks_here.push(Hook {
                    callee: hooks.cover.expect("checked above"),
                    over: Vec::new(),
                    arguments: vec![build.number(id)],
                    before: Vec::new(),
                });
            }
            What::Cover => {
                site("cover", Mode::Count, caller.clone());
                hooks_here.push(Hook {
                    callee: hooks.cover.expect("checked above"),
                    over: Vec::new(),
                    arguments: vec![build.number(id)],
                    before: Vec::new(),
                });
            }
            What::Touch { target, mode } => {
                site("touch", mode, path_of(tcx, target));
                hooks_here.push(Hook {
                    callee: hooks.event,
                    over: Vec::new(),
                    arguments: vec![build.number(id), build.number(mode.code())],
                    before: Vec::new(),
                });
            }
        }
    }

    // Last location first, so a split never moves a location still to come.
    for (at, hooks_here) in at_location.into_iter().rev() {
        insert(tcx, &build, &mut body, at, hooks_here);
    }
    Some((body, sites))
}

/// The calls that write down `captured`, for the next event or frame.
fn argument_hooks<'tcx>(
    tcx: TyCtxt<'tcx>,
    build: &mirth::emit::Build<'tcx>,
    body: &mut Body<'tcx>,
    hooks: &Hooks,
    captured: Captured<'tcx>,
) -> Vec<Hook<'tcx>> {
    let mut out = Vec::new();
    for (operand, how) in captured {
        let ty = operand.ty(&body.local_decls, tcx);
        let (place, before) = materialize(build, body, operand, ty);
        match how {
            How::Text(ty) => out.push(borrow(tcx, build, body, place, ty, hooks.argument, before)),
            How::Debug(ty) => out.push(borrow(
                tcx,
                build,
                body,
                place,
                ty,
                hooks.argument_debug,
                before,
            )),
            How::Plain(leaves) => {
                let parts = leaves.len() as u64;
                let mut before = Some(before);
                for (projection, leaf, own) in leaves {
                    let mut at = place.project_deeper(&projection, tcx);
                    let mut statements = before.take().unwrap_or_default();
                    if own != leaf {
                        let value = new_local(body, leaf);
                        statements.push(build.transmute(value, Operand::Copy(at), leaf));
                        at = Place::from(value);
                    }
                    out.push(borrow(
                        tcx,
                        build,
                        body,
                        at,
                        leaf,
                        hooks.argument,
                        statements,
                    ));
                }
                if parts > 1 {
                    out.push(Hook {
                        callee: hooks.join,
                        over: Vec::new(),
                        arguments: vec![build.number(parts)],
                        before: Vec::new(),
                    });
                }
            }
        }
    }
    out
}

/// A call to `callee::<ty>(&of)`, after `before`.
fn borrow<'tcx>(
    tcx: TyCtxt<'tcx>,
    build: &mirth::emit::Build<'tcx>,
    body: &mut Body<'tcx>,
    of: Place<'tcx>,
    ty: Ty<'tcx>,
    callee: DefId,
    mut before: Vec<Statement<'tcx>>,
) -> Hook<'tcx> {
    let reference = new_local(body, Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, ty));
    before.push(build.reference(reference, of));
    Hook {
        callee,
        over: vec![ty],
        arguments: vec![build.copy(reference)],
        before,
    }
}

/// The value of an operand in a place that can be borrowed.
fn materialize<'tcx>(
    build: &mirth::emit::Build<'tcx>,
    body: &mut Body<'tcx>,
    operand: Operand<'tcx>,
    ty: Ty<'tcx>,
) -> (Place<'tcx>, Vec<Statement<'tcx>>) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => (place, Vec::new()),
        constant => {
            let local = new_local(body, ty);
            (Place::from(local), vec![build.assign(local, constant)])
        }
    }
}

fn new_local<'tcx>(body: &mut Body<'tcx>, ty: Ty<'tcx>) -> Local {
    let span = body.span;
    body.local_decls.push(LocalDecl::new(ty, span))
}

/// Put `hooks` in front of `at`, one call per block, in order.
fn insert<'tcx>(
    tcx: TyCtxt<'tcx>,
    build: &mirth::emit::Build<'tcx>,
    body: &mut Body<'tcx>,
    at: Location,
    hooks: Vec<Hook<'tcx>>,
) {
    let mut current: BasicBlock = at.block;
    let rest = mirth::emit::split(body, at);
    let count = hooks.len();
    for (n, hook) in hooks.into_iter().enumerate() {
        let next = if n + 1 == count {
            rest
        } else {
            let is_cleanup = body.basic_blocks[current].is_cleanup;
            body.basic_blocks_mut()
                .push(rustc_middle::mir::BasicBlockData::new(None, is_cleanup))
        };
        let unit = new_local(body, tcx.types.unit);
        let block = &mut body.basic_blocks_mut()[current];
        block.statements.extend(hook.before);
        block.terminator =
            Some(build.call_over(hook.callee, &hook.over, hook.arguments, unit, next));
        current = next;
    }
}

fn describe(tcx: TyCtxt<'_>, span: rustc_span::Span) -> String {
    let span = span.source_callsite();
    let source_map = tcx.sess.source_map();
    let at = source_map.lookup_char_pos(span.lo());
    format!(
        "{}:{}:{}",
        at.file.name.prefer_remapped_unconditionally(),
        at.line,
        at.col.0 + 1
    )
}

fn snippet(tcx: TyCtxt<'_>, span: rustc_span::Span) -> String {
    let span = span.source_callsite();
    let text = tcx
        .sess
        .source_map()
        .span_to_snippet(span)
        .unwrap_or_default();
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    line.chars().take(160).collect()
}

/// Write the table of sites: one line per site, tab-separated.
pub fn write(directory: &Path, krate: &str, sites: &[Site]) {
    if sites.is_empty() {
        return;
    }
    let _ = std::fs::create_dir_all(directory);
    let mut text = String::new();
    for site in sites {
        let _ = writeln!(
            text,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            site.id,
            site.kind,
            site.mode.name(),
            krate,
            site.caller,
            site.target,
            site.span,
            site.snippet.replace('\t', " "),
        );
    }
    let name = format!("{krate}.sites");
    if let Err(error) = std::fs::write(directory.join(name), text) {
        eprintln!("mirth-watch: writing the site table: {error}");
    }
}

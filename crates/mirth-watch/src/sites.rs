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
use rustc_middle::ty::{GenericArgs, Ty, TyCtxt, TypingEnv};
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
/// path> <fn, extern or const>` (`-` for a body implementing no trait item; `extern` for a
/// function with a foreign ABI, `const` for a constant's or a static's initializer), then
/// `edge <caller hash> <callee hash> <kind> <callee path>`, kind
/// `call` (a direct call, the callee as written: a trait item for a call through a trait, plus
/// the closure or function item a call through `Fn*` names), `resolved` (the implementation a
/// trait call resolves to in the caller's context, where it does),
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
    let mut out = vec![format!(
        "body\t{caller}\t{}\t{}\t{}\t{kind}",
        path_of(tcx, def_id.to_def_id()),
        implements.map_or("-".to_string(), hash),
        implements.map_or("-".to_string(), |it| defining_path_of(tcx, it)),
    )];
    let mut edges = std::collections::HashSet::new();
    struct Refs<'a, 'tcx> {
        tcx: TyCtxt<'tcx>,
        typing_env: TypingEnv<'tcx>,
        edges: &'a mut std::collections::HashSet<(DefId, &'static str)>,
    }
    impl<'tcx> Visitor<'tcx> for Refs<'_, 'tcx> {
        fn visit_terminator(&mut self, terminator: &rustc_middle::mir::Terminator<'tcx>, at: Location) {
            if let TerminatorKind::Call { func, args, .. } = &terminator.kind {
                if let Some((target, generic_args)) = func.const_fn_def() {
                    self.edges.insert((target, "call"));
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
            if let Some((target, _)) = operand.const_fn_def() {
                self.edges.insert((target, "ref"));
            }
            self.super_operand(operand, at);
        }
        fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, at: Location) {
            if let Rvalue::Aggregate(kind, _) = rvalue {
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
    Refs { tcx, typing_env, edges: &mut edges }.visit_body(body);
    // Promoted constants (`&[f, g]`, `&(f as fn())`) have their own bodies.
    for promoted in tcx.promoted_mir(def_id.to_def_id()).iter() {
        Refs { tcx, typing_env, edges: &mut edges }.visit_body(promoted);
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
    lines.sort();
    out.extend(lines);
    out
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

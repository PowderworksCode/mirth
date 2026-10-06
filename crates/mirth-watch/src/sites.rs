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
        })
    }
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
/// One whose type cannot be is left out.
fn captured<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    operands: &[Operand<'tcx>],
    capture: &[usize],
    debug: &[usize],
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
            let operand = operands.get(index)?.clone();
            let how = capture::how(tcx, operand.ty(body, tcx), debug)?;
            Some((operand, how))
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
        let captured = captured(self.tcx, self.body, &operands, &call.capture, &call.debug);
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
        let captured = captured(tcx, original, &parameters, &frame.capture, &frame.debug);
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
        let mut borrow = |of: Place<'tcx>, of_ty: Ty<'tcx>, callee, mut before: Vec<_>| {
            let reference = new_local(body, Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, of_ty));
            before.push(build.reference(reference, of));
            Hook {
                callee,
                over: vec![of_ty],
                arguments: vec![build.copy(reference)],
                before,
            }
        };
        match how {
            How::Text(ty) => out.push(borrow(place, ty, hooks.argument, before)),
            How::Debug(ty) => out.push(borrow(place, ty, hooks.argument_debug, before)),
            How::Plain(leaves) => {
                let parts = leaves.len() as u64;
                let mut before = Some(before);
                for (projection, leaf) in leaves {
                    let at = place.project_deeper(&projection, tcx);
                    out.push(borrow(
                        at,
                        leaf,
                        hooks.argument,
                        before.take().unwrap_or_default(),
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
    let name = format!("{krate}-{}.sites", std::process::id());
    if let Err(error) = std::fs::write(directory.join(name), text) {
        eprintln!("mirth-watch: writing the site table: {error}");
    }
}

//! Being a `rustc` that Cargo will accept, so a plugin does not have to be.
//!
//! Everything here is scaffolding: the argument fixup that lets one binary be
//! both `RUSTC=` and a wrapper, the `--sysroot` that has to be said outright,
//! the `Callbacks` impl, and the global that exists because `override_queries`
//! takes a plain `fn` pointer with no room for state.
//!
//! None of it is a decision. A plugin implements [`Plugin`], calls [`run`],
//! and never mentions `rustc_driver`.

use std::path::PathBuf;
use std::sync::Mutex;

use rustc_middle::mir::Body;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::LocalDefId;

/// What a plugin does to a compilation.
///
/// Every method has a default that does nothing, so a plugin implements the
/// one or two it cares about. The order they run in is the compiler's:
/// `before_lowering` sees the AST before names are resolved, `body` is asked
/// for each function as codegen reaches it, and `finished` runs once the crate
/// is analysed.
pub trait Plugin: Send {
    /// A crate to put in the graph that nothing in the source asked for.
    ///
    /// The path to an `.rlib`. See [`run`] for what is done with it, which is
    /// the part that is not guessable.
    fn injects(&self) -> Option<PathBuf> {
        None
    }

    /// The AST, before it is lowered and before `definitions` is frozen.
    ///
    /// The only point at which new items can be created. By the time MIR is
    /// being asked for, `create_def` is an ICE.
    fn before_lowering(&mut self, _krate: &mut rustc_ast::ast::Crate) {}

    /// One function's MIR, as the compiler produced it.
    ///
    /// Return `Some` to replace it and `None` to leave it alone — and prefer
    /// `None` to returning an untouched clone, so that a body nobody wanted is
    /// the body the compiler made rather than a copy of it.
    ///
    /// Allocate the replacement in `tcx.arena`.
    fn body<'tcx>(
        &mut self,
        _tcx: TyCtxt<'tcx>,
        _def_id: LocalDefId,
        _body: &'tcx Body<'tcx>,
    ) -> Option<&'tcx Body<'tcx>> {
        None
    }

    /// Once every body has been through [`Plugin::body`]. Where a plugin
    /// reports what it did.
    fn finished(&mut self, _tcx: TyCtxt<'_>) {}
}

/// The plugin, reachable from a `fn` pointer.
///
/// `override_queries` hands the provider a plain function with nowhere to put
/// state, so the plugin has to be found rather than passed. A `Mutex` and not
/// a thread-local: `optimized_mir` is asked from whichever thread the compiler
/// feels like. One compilation per process is what makes a single slot safe.
static PLUGIN: Mutex<Option<Box<dyn Plugin>>> = Mutex::new(None);

fn with_plugin<T>(work: impl FnOnce(&mut (dyn Plugin + '_)) -> T) -> Option<T> {
    let mut held = PLUGIN.lock().unwrap_or_else(|it| it.into_inner());
    let plugin = held.as_mut()?;
    Some(work(&mut **plugin))
}

struct Scaffolding;

impl rustc_driver::Callbacks for Scaffolding {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        config.override_queries = Some(|_session, providers| {
            providers.queries.optimized_mir = optimized_mir;
        });
    }

    fn after_crate_root_parsing(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        krate: &mut rustc_ast::ast::Crate,
    ) -> rustc_driver::Compilation {
        with_plugin(|plugin| plugin.before_lowering(krate));
        rustc_driver::Compilation::Continue
    }

    /// Ask for the MIR of every function before the plugin is told the
    /// crate is finished.
    ///
    /// Not an optimisation — it is what makes `finished` mean anything.
    /// `after_analysis` runs *before* codegen, and `optimized_mir` is
    /// asked during it, so a plugin whose `body` had not been called yet
    /// would report an empty tally and be right to. The compiler works
    /// each body out once however many times it is asked, so this is the
    /// same work, earlier and in an order that does not depend on what
    /// codegen happened to reach first.
    fn after_analysis(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'_>,
    ) -> rustc_driver::Compilation {
        for def_id in tcx.mir_keys(()) {
            if is_a_function(tcx, *def_id) {
                tcx.ensure_ok().optimized_mir(def_id.to_def_id());
            }
        }
        with_plugin(|plugin| plugin.finished(tcx));
        rustc_driver::Compilation::Continue
    }
}

/// Whether a definition is something `optimized_mir` may be asked for.
///
/// Two conditions, and the second is not optional. `DefKind` says it is a
/// function rather than a `const` or a `static`. `hir_body_const_context` says
/// it is not being evaluated at compile time — which is rustc's own
/// precondition, asserted in `rustc_mir_transform`:
///
/// ```text
/// do not use `optimized_mir` for constants: Const { allow_const_fn_promotion: false }
/// ```
///
/// Asking anyway brings the compiler down, and nothing warns first. Found by
/// compiling `core` from source with `-Zbuild-std`, which is full of bodies
/// that pass the first test and fail the second; no ordinary crate in the
/// corpus had one.
pub fn is_a_function(tcx: TyCtxt<'_>, def_id: LocalDefId) -> bool {
    matches!(
        tcx.def_kind(def_id),
        rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::AssocFn
    ) && tcx.hir_body_const_context(def_id).is_none()
}

/// The provider, which is where both halves of a MIR plugin have to live.
///
/// The decision needs the MIR as the compiler produced it and the rewrite has
/// to be what the compiler gets back. Deciding anywhere else would either read
/// a body the plugin had already changed, or file an unchanged one under the
/// name of the changed one.
fn optimized_mir<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> &'tcx Body<'tcx> {
    let made = (rustc_interface::DEFAULT_QUERY_PROVIDERS
        .queries
        .optimized_mir)(tcx, def_id);
    with_plugin(|plugin| plugin.body(tcx, def_id, made))
        .flatten()
        .unwrap_or(made)
}

/// Run a plugin as `rustc`.
///
/// `sysroot` is what the plugin's `build.rs` recorded through
/// `mirth_build::link_to_the_toolchain`, read with `env!("MIRTH_SYSROOT")`.
/// It is passed rather than looked up because a plugin *is* the compiler it
/// was built against, and one that asked `rustc --print sysroot` on each
/// invocation would answer with whatever toolchain happened to be active in
/// the directory it was compiling.
///
/// Does not return.
///
/// Cargo invokes a wrapper as `<wrapper> <rustc> <args…>`, so the real
/// compiler's path arrives as the first argument, and rustc's own parser
/// would read it as a source file. Dropping it is what makes one binary
/// usable both as `RUSTC=` and as a wrapper.
///
/// rustc locates its sysroot from wherever `librustc_driver` was loaded,
/// which the rpath points at the pinned toolchain. That works, but it is
/// inference; saying it outright means a plugin copied elsewhere, or run
/// with an odd loader configuration, still compiles against the standard
/// library it was built against rather than failing obscurely.
pub fn run(sysroot: &str, plugin: impl Plugin + 'static) -> ! {
    let mut args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && is_rustc(&args[1]) {
        args.remove(1);
    }

    if !args.iter().any(|arg| arg.starts_with("--sysroot")) {
        args.push("--sysroot".to_owned());
        args.push(sysroot.to_owned());
    }

    if let Some(rlib) = plugin.injects() {
        inject(&mut args, &rlib);
    }

    *PLUGIN.lock().unwrap_or_else(|it| it.into_inner()) = Some(Box::new(plugin));
    rustc_driver::run_compiler(&args, &mut Scaffolding);
    std::process::exit(0)
}

/// Put a crate in the graph that nothing in the source refers to.
///
/// `force:` is what makes it arrive at all: an `--extern` for a crate nobody
/// named is otherwise dropped as unused. And `-L dependency=` beside it is not
/// optional — without it, rustc looks for the injected crate's own `serde` in
/// the sysroot, finds the copy the compiler ships, and rejects it as a
/// different crate. The error names neither the program nor the plugin.
fn inject(args: &mut Vec<String>, rlib: &std::path::Path) {
    let Some(name) = rlib
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.strip_prefix("lib"))
    else {
        eprintln!("mirth: {} is not a library name", rlib.display());
        return;
    };
    if !rlib.is_file() {
        eprintln!("mirth: nothing at {}; compiling without it", rlib.display());
        return;
    }
    args.push("-Zunstable-options".to_owned());
    args.push("--extern".to_owned());
    args.push(format!("force:{name}={}", rlib.display()));
    if let Some(beside) = rlib.parent() {
        args.push("-L".to_owned());
        args.push(format!("dependency={}", beside.join("deps").display()));
    }
}

/// Whether an argument is the compiler a wrapper was handed rather than a
/// source file.
fn is_rustc(argument: &str) -> bool {
    std::path::Path::new(argument)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == "rustc")
}

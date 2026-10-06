//! Running a plugin as `rustc`: argument handling, the driver callbacks, and
//! the `optimized_mir` override.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rustc_middle::mir::Body;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::LocalDefId;

/// What a plugin does to a compilation. Every method defaults to doing
/// nothing.
///
/// The compiler calls them in this order: `before_lowering` with the AST,
/// `body` once for each function, then `finished`.
pub trait Plugin: Send {
    /// An `.rlib` to add to the crate graph, though the source never names
    /// it. See [`run`].
    fn injects(&self) -> Option<PathBuf> {
        None
    }

    /// Flags to add to this compilation's own.
    fn arguments(&self) -> Vec<String> {
        Vec::new()
    }

    /// The AST, before name resolution. New items can only be added here:
    /// creating a definition once MIR is being built is an ICE.
    fn before_lowering(&mut self, _krate: &mut rustc_ast::ast::Crate) {}

    /// One function's optimized MIR. Return a replacement allocated in
    /// `tcx.arena`, or `None` to keep the compiler's own.
    fn body<'tcx>(
        &mut self,
        _tcx: TyCtxt<'tcx>,
        _def_id: LocalDefId,
        _body: &'tcx Body<'tcx>,
    ) -> Option<&'tcx Body<'tcx>> {
        None
    }

    /// After every function's body has been through [`Plugin::body`].
    fn finished(&mut self, _tcx: TyCtxt<'_>) {}
}

/// The plugin, where the `optimized_mir` provider can reach it.
/// `override_queries` takes a plain `fn` pointer, so there is no other way
/// to pass it. A `Mutex` because the query can run on any thread.
static PLUGIN: Mutex<Option<Box<dyn Plugin>>> = Mutex::new(None);

fn with_plugin<T>(work: impl FnOnce(&mut (dyn Plugin + '_)) -> T) -> Option<T> {
    let mut held = PLUGIN.lock().unwrap_or_else(|it| it.into_inner());
    let plugin = held.as_mut()?;
    Some(work(&mut **plugin))
}

struct Callbacks;

impl rustc_driver::Callbacks for Callbacks {
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

    /// Analysis finishes before code generation asks for most bodies, so
    /// the MIR of every function and closure is requested here first.
    /// Otherwise `finished` would run before the plugin had seen them.
    fn after_analysis(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'_>,
    ) -> rustc_driver::Compilation {
        for def_id in tcx.mir_keys(()) {
            if has_optimized_mir(tcx, *def_id) {
                tcx.ensure_ok().optimized_mir(def_id.to_def_id());
            }
        }
        with_plugin(|plugin| plugin.finished(tcx));
        rustc_driver::Compilation::Continue
    }
}

/// Whether `optimized_mir` may be requested for a definition: a function or
/// closure that is not evaluated at compile time. rustc asserts the second
/// part.
pub fn has_optimized_mir(tcx: TyCtxt<'_>, def_id: LocalDefId) -> bool {
    use rustc_hir::def::DefKind;
    matches!(
        tcx.def_kind(def_id),
        DefKind::Fn | DefKind::AssocFn | DefKind::Closure | DefKind::SyntheticCoroutineBody
    ) && tcx.hir_body_const_context(def_id).is_none()
}

/// The `optimized_mir` provider: the compiler's body, then the plugin's
/// replacement for it if it has one. Codegen gets whatever this returns.
fn optimized_mir<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> &'tcx Body<'tcx> {
    let made = (rustc_interface::DEFAULT_QUERY_PROVIDERS
        .queries
        .optimized_mir)(tcx, def_id);
    with_plugin(|plugin| plugin.body(tcx, def_id, made))
        .flatten()
        .unwrap_or(made)
}

/// Run `plugin` as `rustc`, with this process's arguments. Does not return.
///
/// - `sysroot` is the toolchain the plugin was built against, from
///   `env!("MIRTH_SYSROOT")`. It is passed as `--sysroot` unless the
///   arguments already have one.
/// - Cargo runs a wrapper as `<wrapper> <rustc> <args…>`. A leading argument
///   that names `rustc` is dropped, so the same binary works as a wrapper and
///   as `RUSTC=`.
/// - An `.rlib` from [`Plugin::injects`] is added with `--extern force:`,
///   which loads a crate the source never names. Its directory and its
///   `deps` directory go on the search path: rustc finds a dependency of a
///   dependency by searching, not through `--extern`.
pub fn run(sysroot: &str, plugin: impl Plugin + 'static) -> ! {
    let mut args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && is_rustc(&args[1]) {
        args.remove(1);
    }

    if !args.iter().any(|arg| arg.starts_with("--sysroot")) {
        args.push("--sysroot".to_owned());
        args.push(sysroot.to_owned());
    }

    args.extend(plugin.arguments());

    if let Some(rlib) = plugin.injects() {
        inject(&mut args, &rlib);
    }

    *PLUGIN.lock().unwrap_or_else(|it| it.into_inner()) = Some(Box::new(plugin));
    rustc_driver::compiler_entrypoint(&args, &mut Callbacks);
    std::process::exit(0)
}

fn inject(args: &mut Vec<String>, rlib: &Path) {
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
        for directory in [beside.to_path_buf(), beside.join("deps")] {
            args.push("-L".to_owned());
            args.push(format!("dependency={}", directory.display()));
        }
    }
}

/// Whether an argument is the path of a compiler, not a source file.
fn is_rustc(argument: &str) -> bool {
    Path::new(argument)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == "rustc")
}

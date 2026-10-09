//! A mirth plugin that records the side effects a configuration names.
//!
//! Run as a `rustc` wrapper with `MIRTH_WATCH` naming a configuration
//! (`config.rs` documents it) and `MIRTH_RUNTIME` naming
//! `libmirth_runtime.rlib`. Each crate in scope gets calls to the runtime at
//! every site the configuration names, and the compilation writes a table of
//! those sites to `<crate>.sites` in the directory `MIRTH_SITES` names: what
//! each site's number means, for whoever reads the logs the program writes
//! when it runs. A crate compiled again replaces its table.

#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_trait_selection;

mod capture;
mod config;
mod sites;

use std::path::PathBuf;

use config::Config;
use rustc_middle::mir::Body;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::{LOCAL_CRATE, LocalDefId};

struct Watch {
    config: Config,
    runtime: PathBuf,
    sites: Vec<sites::Site>,
    paths: std::collections::BTreeSet<String>,
    graph: Vec<String>,
}

impl mirth::Plugin for Watch {
    fn injects(&self) -> Option<PathBuf> {
        Some(self.runtime.clone())
    }

    /// The MIR inliner runs before `optimized_mir` hands a body over, and
    /// would replace a call to a small function, such as `std::fs::rename`,
    /// with that function's body and its own calls. Code generation still
    /// inlines afterwards.
    fn arguments(&self) -> Vec<String> {
        vec!["-Zinline-mir=no".to_owned()]
    }

    fn body<'tcx>(
        &mut self,
        tcx: TyCtxt<'tcx>,
        def_id: LocalDefId,
        body: &'tcx Body<'tcx>,
    ) -> Option<&'tcx Body<'tcx>> {
        if self.config.diagnostics.paths {
            self.paths
                .insert(format!("body\t{}", sites::path_of(tcx, def_id.to_def_id())));
            self.paths.extend(
                sites::callees(tcx, body)
                    .into_iter()
                    .map(|path| format!("call\t{path}")),
            );
        }
        if self.config.diagnostics.callgraph {
            self.graph.extend(sites::graph(tcx, def_id, body, "fn"));
        }
        let hooks = sites::Hooks::find(tcx)?;
        let (changed, found) = sites::instrument(tcx, &self.config, &hooks, def_id, body)?;
        self.sites.extend(found);
        Some(tcx.arena.alloc(changed))
    }

    fn finished(&mut self, tcx: TyCtxt<'_>) {
        if sites::Hooks::find(tcx).is_none() {
            eprintln!("mirth-watch: the runtime's hooks are missing; nothing was instrumented");
        }
        let Some(directory) = std::env::var_os("MIRTH_SITES") else {
            return;
        };
        let directory = PathBuf::from(directory);
        let krate = tcx.crate_name(LOCAL_CRATE);
        if self.config.diagnostics.callgraph {
            // Constants' and statics' initializers have no `optimized_mir`; the functions they
            // name (tables of function pointers, callbacks) are references too.
            use rustc_hir::def::DefKind;
            for owner in tcx.hir_body_owners() {
                if matches!(
                    tcx.def_kind(owner),
                    DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::AnonConst | DefKind::Static { .. }
                ) {
                    let body = tcx.mir_for_ctfe(owner.to_def_id());
                    self.graph.extend(sites::graph(tcx, owner, body, "const"));
                }
            }
        }
        sites::write(&directory, krate.as_str(), &self.sites);
        if !self.graph.is_empty() {
            let _ = std::fs::create_dir_all(&directory);
            let _ = std::fs::write(directory.join(format!("{krate}.graph")), self.graph.join("\n") + "\n");
        }
        if !self.paths.is_empty() {
            let mut text = self.paths.iter().cloned().collect::<Vec<_>>().join("\n");
            text.push('\n');
            let _ = std::fs::create_dir_all(&directory);
            let _ = std::fs::write(directory.join(format!("{krate}.paths")), text);
        }
    }
}

/// For crates out of scope: compile as plain rustc would, with the
/// runtime's directory on the search path, for a dependency that was
/// instrumented and links it.
struct Untouched {
    runtime: Option<PathBuf>,
}

impl mirth::Plugin for Untouched {
    fn arguments(&self) -> Vec<String> {
        match self.runtime.as_ref().and_then(|it| it.parent()) {
            Some(directory) => vec![
                "-L".to_owned(),
                format!("dependency={}", directory.display()),
            ],
            None => Vec::new(),
        }
    }
}

/// The standard library and what it is built from. The runtime is built on
/// `std`, so it cannot be put inside it.
const BENEATH_THE_RUNTIME: [&str; 22] = [
    "core",
    "alloc",
    "std",
    "compiler_builtins",
    "panic_abort",
    "panic_unwind",
    "unwind",
    "libc",
    "std_detect",
    "hashbrown",
    "cfg_if",
    "rustc_std_workspace_core",
    "rustc_std_workspace_alloc",
    "rustc_std_workspace_std",
    "addr2line",
    "gimli",
    "object",
    "memchr",
    "miniz_oxide",
    "adler2",
    "rustc_demangle",
    "mirth_runtime",
];

fn crate_name() -> Option<String> {
    let arguments: Vec<String> = std::env::args().collect();
    let at = arguments.iter().position(|it| it == "--crate-name")?;
    arguments.get(at + 1).cloned()
}

fn main() -> ! {
    let sysroot = env!("MIRTH_SYSROOT");
    let runtime = std::env::var_os("MIRTH_RUNTIME").map(PathBuf::from);
    let config = Config::load();
    let name = crate_name();

    let beneath = name
        .as_deref()
        .is_some_and(|name| BENEATH_THE_RUNTIME.contains(&name));
    let in_scope = match (&config, &name) {
        (Some(config), Some(name)) if config.scope.crates.is_empty() => {
            !mirth::config::a_dependency()
        }
        (Some(config), Some(name)) => config.scope.crates.iter().any(|it| config::matches(it, name)),
        _ => false,
    };

    match (config, runtime) {
        (Some(config), Some(runtime)) if in_scope && !beneath => mirth::run(
            sysroot,
            Watch {
                config,
                runtime,
                sites: Vec::new(),
                paths: Default::default(),
                graph: Vec::new(),
            },
        ),
        (_, runtime) => mirth::run(
            sysroot,
            Untouched {
                runtime: runtime.filter(|_| !beneath),
            },
        ),
    }
}

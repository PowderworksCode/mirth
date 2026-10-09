//! `cargo mirth`: run a Cargo command with mirth-watch instrumenting it.
//!
//! ```text
//! cargo mirth build [cargo arguments]
//! cargo mirth run [cargo arguments] [-- program arguments]
//! cargo mirth test [cargo arguments]
//! ```
//!
//! What to watch comes from `MIRTH_WATCH`, or else from the nearest
//! `mirth.toml` above the current directory. Everything mirth writes goes
//! under the workspace's `target/mirth`:
//!
//! - `target/` is the build. It is kept apart from the ordinary one because
//!   Cargo does not notice the wrapper or the configuration changing, and
//!   would mix instrumented and plain artifacts;
//! - `runtime/` holds the runtime, compiled from source with the toolchain
//!   the plugin was built against;
//! - `sites/` gets the site tables, written while compiling;
//! - `logs/` gets one log per process, written while running.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const RUNTIME: [(&str, &str); 4] = [
    ("lib.rs", include_str!("../../../mirth-runtime/src/lib.rs")),
    (
        "capture.rs",
        include_str!("../../../mirth-runtime/src/capture.rs"),
    ),
    ("log.rs", include_str!("../../../mirth-runtime/src/log.rs")),
    ("coverage.rs", include_str!("../../../mirth-runtime/src/coverage.rs")),
];

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("cargo mirth: {message}");
    ExitCode::from(2)
}

fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// The workspace root, as Cargo locates it.
fn workspace() -> Result<PathBuf, String> {
    let located = Command::new(cargo())
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .output()
        .map_err(|error| format!("running cargo: {error}"))?;
    if !located.status.success() {
        return Err(String::from_utf8_lossy(&located.stderr).trim().to_owned());
    }
    let manifest = PathBuf::from(String::from_utf8_lossy(&located.stdout).trim());
    Ok(manifest.parent().map(Path::to_path_buf).unwrap_or_default())
}

fn configuration() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MIRTH_WATCH") {
        return Ok(PathBuf::from(path));
    }
    let mut directory = std::env::current_dir().map_err(|error| error.to_string())?;
    loop {
        let candidate = directory.join("mirth.toml");
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !directory.pop() {
            return Err("no mirth.toml here or above, and MIRTH_WATCH is not set".to_owned());
        }
    }
}

/// The runtime, compiled with the plugin's own toolchain. rustc on its own
/// keeps the full metadata in the rlib, so it can be injected as one file.
fn runtime(directory: &Path) -> Result<PathBuf, String> {
    let source = directory.join("src");
    std::fs::create_dir_all(&source).map_err(|error| error.to_string())?;
    let mut changed = false;
    for (name, text) in RUNTIME {
        let path = source.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text) {
            std::fs::write(&path, text).map_err(|error| error.to_string())?;
            changed = true;
        }
    }
    let rlib = directory.join("libmirth_runtime.rlib");
    if changed || !rlib.is_file() {
        let rustc = Path::new(env!("MIRTH_SYSROOT")).join("bin").join("rustc");
        let built = Command::new(rustc)
            .args(["--edition", "2024", "--crate-type", "rlib"])
            .args(["--crate-name", "mirth_runtime", "-O"])
            .arg(source.join("lib.rs"))
            .arg("--out-dir")
            .arg(directory)
            .env("RUSTC_BOOTSTRAP", "1")
            .output()
            .map_err(|error| format!("compiling the runtime: {error}"))?;
        if !built.status.success() {
            return Err(format!(
                "compiling the runtime:\n{}",
                String::from_utf8_lossy(&built.stderr)
            ));
        }
    }
    Ok(rlib)
}

fn main() -> ExitCode {
    let mut arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    if arguments.first().is_some_and(|it| it == "mirth") {
        arguments.remove(0);
    }
    let Some(command) = arguments.first().cloned() else {
        return fail("give a cargo command: build, run, test, …");
    };

    let plugin = match std::env::current_exe() {
        Ok(path) => path.with_file_name(format!("mirth-watch{}", std::env::consts::EXE_SUFFIX)),
        Err(error) => return fail(error),
    };
    if !plugin.is_file() {
        return fail(format!(
            "mirth-watch is not beside cargo-mirth at {}",
            plugin.display()
        ));
    }
    let watch = match configuration() {
        Ok(path) => path,
        Err(error) => return fail(error),
    };
    let root = match workspace() {
        Ok(root) => root.join("target").join("mirth"),
        Err(error) => return fail(error),
    };
    let runtime = match runtime(&root.join("runtime")) {
        Ok(rlib) => rlib,
        Err(error) => return fail(error),
    };

    let mut cargo = Command::new(cargo());
    cargo
        .args(&arguments)
        .env("RUSTC_WRAPPER", &plugin)
        .env("MIRTH_WATCH", &watch)
        .env("MIRTH_RUNTIME", &runtime)
        .env("MIRTH_SITES", root.join("sites"));
    if std::env::var_os("CARGO_TARGET_DIR").is_none() {
        cargo.env("CARGO_TARGET_DIR", root.join("target"));
    }
    if command == "run" || command == "test" {
        cargo.env("MIRTH_OUT", root.join("logs"));
    }
    // Windows finds the compiler's dynamic libraries through PATH.
    if cfg!(windows) {
        let bin = Path::new(env!("MIRTH_SYSROOT")).join("bin");
        let mut paths = vec![bin];
        if let Some(path) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&path));
        }
        if let Ok(path) = std::env::join_paths(paths) {
            cargo.env("PATH", path);
        }
    }
    match cargo.status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(1, 255) as u8),
        Err(error) => fail(format!("running cargo: {error}")),
    }
}

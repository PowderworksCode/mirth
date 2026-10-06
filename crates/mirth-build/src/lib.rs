//! What a mirth plugin's build script has to do.
//!
//! ```no_run
//! mirth_build::link_to_the_toolchain();
//! ```
//!
//! A separate crate from `mirth` because a build script cannot depend on a
//! crate that links `rustc_private`: the compiler's own libraries are not
//! available to the build script's host compilation. Nothing here needs them.
//! The whole job is to ask a compiler where it lives and write that down.

use std::process::Command;

/// Record the active toolchain, so the plugin can start and can compile
/// against the standard library it was built against.
///
/// - `MIRTH_SYSROOT` and `MIRTH_TOOLCHAIN` become compile-time environment,
///   readable with `env!` from the plugin's source. The sysroot is baked in
///   rather than looked up when the plugin runs: the plugin *is* the compiler
///   it was built against, and asking `rustc --print sysroot` at run time
///   would answer with whatever toolchain is active where it was started.
///
/// - On Linux and macOS, an rpath into the sysroot's `lib`. A rustc driver
///   loads the compiler's own dynamic libraries (`librustc_driver-*`, the
///   toolchain's `libstd-*`) and nothing on the default search path provides
///   them, so without it the binary does not start. Windows has no rpath: the
///   loader searches `PATH`, and the sysroot's `bin` has to be on it. Cargo
///   puts it there for `cargo run` and `cargo test`.
pub fn link_to_the_toolchain() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());

    let sysroot = ask(&rustc, "--print", Some("sysroot"), "its sysroot");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{sysroot}/lib");
    }
    println!("cargo:rustc-env=MIRTH_SYSROOT={sysroot}");

    let version = ask(&rustc, "--version", None, "its version");
    println!("cargo:rustc-env=MIRTH_TOOLCHAIN={version}");

    println!("cargo:rerun-if-changed=build.rs");
}

fn ask(rustc: &str, flag: &str, value: Option<&str>, what: &str) -> String {
    let mut command = Command::new(rustc);
    command.arg(flag);
    if let Some(value) = value {
        command.arg(value);
    }
    let said = command
        .output()
        .unwrap_or_else(|error| panic!("asking the active toolchain for {what}: {error}"));
    String::from_utf8(said.stdout)
        .unwrap_or_else(|_| panic!("{what} is not text"))
        .trim()
        .to_owned()
}

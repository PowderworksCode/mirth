//! What a mirth plugin's build script has to do:
//!
//! ```no_run
//! mirth_build::link_to_the_toolchain();
//! ```
//!
//! This is separate from `mirth` because a build script cannot depend on a
//! crate that links `rustc_private`.

use std::process::Command;

/// Record the active toolchain for the plugin being built.
///
/// - Sets `MIRTH_SYSROOT` and `MIRTH_TOOLCHAIN` for `env!` in the plugin's
///   source. The sysroot is fixed at build time because the plugin must use
///   the standard library it was built against, whatever toolchain is active
///   when it runs.
/// - On Linux and macOS, adds an rpath to the sysroot's `lib`, where the
///   compiler's dynamic libraries are; without it the plugin does not start.
///   Windows finds them through `PATH`, which Cargo sets for `cargo run` and
///   `cargo test`.
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

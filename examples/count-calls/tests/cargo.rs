//! The plugin driven by Cargo, the way a user would run it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn plugin() -> &'static str {
    env!("CARGO_BIN_EXE_count-calls")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn cargo_build(fixture: &Path, target: &Path) -> Output {
    Command::new(env!("CARGO"))
        .arg("build")
        .arg("--manifest-path")
        .arg(fixture.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", target)
        .env("RUSTC_WRAPPER", plugin())
        .output()
        .expect("running cargo")
}

#[test]
fn counts_the_primary_package_and_leaves_dependencies_alone() {
    let target = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("two-crates");
    let built = cargo_build(&fixture("two-crates"), &target);
    let stderr = text(&built.stderr);
    assert!(built.status.success(), "cargo build failed:\n{stderr}");
    assert!(stderr.contains("count-calls: `app`:"), "{stderr}");
    assert!(!stderr.contains("count-calls: `dep`"), "{stderr}");

    let program = target
        .join("debug")
        .join(format!("app{}", std::env::consts::EXE_SUFFIX));
    let ran = Command::new(program).output().expect("running the program");
    assert_eq!(text(&ran.stdout).trim(), "total 5");
}

#[test]
fn answers_cargos_probe_like_rustc() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let real = Command::new(&rustc).arg("-vV").output().expect("rustc -vV");
    let ours = Command::new(plugin())
        .arg(&rustc)
        .arg("-vV")
        .output()
        .expect("count-calls rustc -vV");
    assert!(ours.status.success(), "{}", text(&ours.stderr));
    assert_eq!(text(&ours.stdout), text(&real.stdout));
}

//! Interpreting a test with Miri (the pinned nightly's, with the sysroot `cargo miri setup`
//! makes).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Serialize;

use crate::rustc::{Exit, run_command};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MiriStatus {
    /// Ran to the end, whatever the exit code.
    Ok,
    Ub,
    Unsupported,
    Error,
    Ice,
    Timeout,
}

#[derive(Clone, Debug, Serialize)]
pub struct MiriRun {
    pub status: MiriStatus,
    pub exit: Exit,
    pub stdout: String,
    pub stderr: String,
}

pub struct Miri {
    pub binary: PathBuf,
    pub sysroot: PathBuf,
}

impl Miri {
    /// The pinned toolchain's Miri and the default `cargo miri setup` sysroot.
    pub fn pinned(toolchain: &str) -> Miri {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        Miri {
            binary: home.join(format!(".rustup/toolchains/{toolchain}-x86_64-unknown-linux-gnu/bin/miri")),
            sysroot: home.join(".cache/miri"),
        }
    }

    pub fn run(&self, source: &Path, flags: &[String], edition: &str, extra: &[String], timeout: u64, cwd: &Path) -> MiriRun {
        let mut cmd = Command::new(&self.binary);
        cmd.arg("--sysroot")
            .arg(&self.sysroot)
            .arg(source)
            .args(["--edition", edition])
            .args(["-Zunstable-options", "-Ainternal_features", "-Aincomplete_features"])
            .args(["-Zmiri-disable-isolation", "-Zmiri-deterministic-floats"])
            .args(flags)
            .args(extra)
            .current_dir(cwd)
            .env("RUSTC_BOOTSTRAP", "1")
            .env("RUST_BACKTRACE", "0");
        let done = match run_command(cmd, Duration::from_secs(timeout)) {
            Ok(d) => d,
            Err(e) => {
                return MiriRun { status: MiriStatus::Error, exit: Exit::Code(-1), stdout: String::new(), stderr: e.to_string() };
            }
        };
        let err = done.stderr_text();
        let status = if done.exit == Exit::Timeout {
            MiriStatus::Timeout
        } else if err.contains("Undefined Behavior:") {
            MiriStatus::Ub
        } else if err.contains("unsupported operation") || err.contains("can't call foreign function") {
            MiriStatus::Unsupported
        } else if crate::rustc::is_ice(&err) {
            MiriStatus::Ice
        } else if done.exit == Exit::Code(1) && !err.contains("panicked") && err.lines().any(|l| l.starts_with("error")) {
            MiriStatus::Error
        } else {
            MiriStatus::Ok
        };
        MiriRun { status, exit: done.exit.clone(), stdout: done.stdout_text(), stderr: err }
    }
}

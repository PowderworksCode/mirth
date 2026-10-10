//! Running rustc (and the programs it builds, and Miri) with timeouts, and reading its JSON
//! diagnostics into types.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

/// How a command ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Exit {
    Code(i32),
    Signal(i32),
    Timeout,
}

/// What a finished command printed.
#[derive(Clone, Debug)]
pub struct Finished {
    pub exit: Exit,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Finished {
    pub fn success(&self) -> bool {
        self.exit == Exit::Code(0)
    }
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

/// Run `cmd` with stdin closed, its output captured, killed after `timeout`.
pub fn run_command(mut cmd: Command, timeout: Duration) -> std::io::Result<Finished> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // A binary just written can be "busy" when another thread forked while it was open for
    // writing (the child holds the descriptor until it execs): retry for a while.
    let mut tries = 0;
    let mut child = loop {
        match cmd.spawn() {
            Err(e) if e.raw_os_error() == Some(26) && tries < 100 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            other => break other?,
        }
    };
    // Read both pipes on threads, so a chatty child cannot block on a full pipe.
    let mut out = child.stdout.take().expect("piped");
    let mut err = child.stderr.take().expect("piped");
    let out_thread = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = out.read_to_end(&mut v);
        v
    });
    let err_thread = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = err.read_to_end(&mut v);
        v
    });
    let exit = match child.wait_timeout(timeout)? {
        Some(status) => match status.code() {
            Some(code) => Exit::Code(code),
            None => {
                use std::os::unix::process::ExitStatusExt;
                Exit::Signal(status.signal().unwrap_or(0))
            }
        },
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Exit::Timeout
        }
    };
    Ok(Finished {
        exit,
        stdout: out_thread.join().unwrap_or_default(),
        stderr: err_thread.join().unwrap_or_default(),
    })
}

pub fn is_ice(stderr: &str) -> bool {
    stderr.contains("internal compiler error")
        || stderr.contains("the compiler unexpectedly panicked")
        || stderr.contains("rustc interrupted by SIG")
}

/// The outcome of a compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Error,
    Ice,
    Timeout,
}

/// A compilation's result: its status, what it printed, and the program if it built one.
#[derive(Clone, Debug)]
pub struct Compiled {
    pub status: Status,
    pub stderr: String,
    pub binary: Option<PathBuf>,
}

/// One rustc invocation, the way the checks compile a test.
pub struct Compile<'a> {
    pub rustc: &'a Path,
    pub source: &'a Path,
    pub out_dir: &'a Path,
    pub flags: &'a [String],
    pub edition: &'a str,
    pub extra: Vec<String>,
    /// `link`, `metadata`, `link,metadata`, ...
    pub emit: &'a str,
    pub json: bool,
    pub timeout: Duration,
    /// Run with `RUSTC_BOOTSTRAP=1` (the default; a stable user's view needs it off).
    pub bootstrap: bool,
    /// Name the output `<out_dir>/prog` with `-o` (the default); off when the extra options say
    /// where outputs go (`--out-dir`).
    pub name_output: bool,
}

impl<'a> Compile<'a> {
    pub fn new(rustc: &'a Path, source: &'a Path, out_dir: &'a Path, flags: &'a [String], edition: &'a str) -> Self {
        Compile {
            rustc,
            source,
            out_dir,
            flags,
            edition,
            extra: Vec::new(),
            emit: "link",
            json: false,
            timeout: Duration::from_secs(300),
            bootstrap: true,
            name_output: true,
        }
    }

    pub fn extra<I: IntoIterator<Item = S>, S: Into<String>>(mut self, extra: I) -> Self {
        self.extra.extend(extra.into_iter().map(Into::into));
        self
    }

    pub fn emit(mut self, emit: &'a str) -> Self {
        self.emit = emit;
        self
    }

    pub fn json(mut self) -> Self {
        self.json = true;
        self
    }

    pub fn unnamed_output(mut self) -> Self {
        self.name_output = false;
        self
    }

    pub fn stable(mut self) -> Self {
        self.bootstrap = false;
        self
    }

    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    pub fn run(self) -> Compiled {
        let _ = std::fs::create_dir_all(self.out_dir);
        let binary = self.out_dir.join("prog");
        let mut cmd = Command::new(self.rustc);
        cmd.arg(self.source)
            .args(["--edition", self.edition])
            .arg(format!("--emit={}", self.emit));
        if self.name_output {
            cmd.arg("-o").arg(&binary);
        }
        if self.bootstrap {
            cmd.args(["-Zunstable-options", "-Ainternal_features", "-Aincomplete_features"]);
        }
        cmd
            .arg(if self.json { "--error-format=json" } else { "--error-format=short" })
            .args(self.flags)
            .args(&self.extra)
            .current_dir(self.out_dir)
            .env("RUST_BACKTRACE", "0");
        if self.bootstrap {
            cmd.env("RUSTC_BOOTSTRAP", "1");
        } else {
            cmd.env_remove("RUSTC_BOOTSTRAP");
        }
        let done = match run_command(cmd, self.timeout) {
            Ok(d) => d,
            Err(e) => {
                return Compiled { status: Status::Error, stderr: format!("spawning rustc: {e}"), binary: None };
            }
        };
        let stderr = done.stderr_text();
        let status = match done.exit {
            Exit::Timeout => Status::Timeout,
            _ if is_ice(&stderr) => Status::Ice,
            Exit::Code(0) => Status::Ok,
            _ => Status::Error,
        };
        let binary = (status == Status::Ok && self.emit.contains("link") && binary.exists()).then_some(binary);
        Compiled { status, stderr, binary }
    }
}

/// What running a built program observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Observed {
    pub exit: Exit,
    pub stdout: String,
    pub stderr: String,
}

/// Run a program built by a check, from its own directory.
pub fn observe(binary: &Path, timeout_secs: u64, env: &[(&str, &str)]) -> Observed {
    let mut cmd = Command::new(binary);
    cmd.current_dir(binary.parent().unwrap_or(Path::new("."))).env("RUST_BACKTRACE", "0");
    for (k, v) in env {
        cmd.env(k, v);
    }
    match run_command(cmd, Duration::from_secs(timeout_secs)) {
        Ok(d) => Observed { exit: d.exit.clone(), stdout: d.stdout_text(), stderr: d.stderr_text() },
        Err(e) => Observed { exit: Exit::Code(-1), stdout: String::new(), stderr: format!("spawning: {e}") },
    }
}

/// One diagnostic of `--error-format=json`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Diagnostic {
    pub message: String,
    #[serde(default)]
    pub code: Option<Code>,
    pub level: String,
    #[serde(default)]
    pub spans: Vec<Span>,
    #[serde(default)]
    pub children: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Code {
    pub code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Span {
    pub file_name: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub suggested_replacement: Option<String>,
    #[serde(default)]
    pub suggestion_applicability: Option<String>,
}

impl Diagnostic {
    pub fn code(&self) -> &str {
        self.code.as_ref().map_or("", |c| c.code.as_str())
    }

    /// This diagnostic and its children, depth first.
    pub fn walk(&self) -> Vec<&Diagnostic> {
        let mut out = vec![self];
        for c in &self.children {
            out.extend(c.walk());
        }
        out
    }

    pub fn primary(&self) -> Option<&Span> {
        self.spans.iter().find(|s| s.is_primary)
    }
}

/// The diagnostics in rustc's JSON stderr (lines that are not JSON are skipped).
pub fn diagnostics(stderr: &str) -> Vec<Diagnostic> {
    stderr.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// The error codes in human-readable stderr, sorted and deduplicated.
pub fn error_codes(stderr: &str) -> Vec<String> {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"error\[(E\d{4})\]").unwrap());
    let mut codes: Vec<String> = RE.captures_iter(stderr).map(|c| c[1].to_owned()).collect();
    codes.sort();
    codes.dedup();
    codes
}

/// The first line of stderr that starts an error, for reports.
pub fn first_error(stderr: &str) -> String {
    stderr.lines().find(|l| l.contains("error")).unwrap_or("").chars().take(300).collect()
}

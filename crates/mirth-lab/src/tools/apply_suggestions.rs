//! Each machine-applicable suggestion of a warning in one file, applied alone: does the result
//! still compile? (Finding 29's reductions; `suggest-diff` does this over every UI test.)
//!
//! Writes `<file>_fix<n>.rs` next to the file for the n-th suggestion and prints, per
//! suggestion, the lint code, the replaced text, the replacements and the first error.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use mirth_lab::rustc::diagnostics;

#[derive(clap::Args, Debug)]
pub struct Args {
    file: PathBuf,
    /// A rustup toolchain.
    toolchain: String,
}

fn check(toolchain: &str, file: &str, json: bool) -> anyhow::Result<String> {
    let mut cmd = Command::new("rustc");
    cmd.arg(format!("+{toolchain}")).args(["--edition", "2021", "--emit=metadata"]);
    if json {
        cmd.arg("--error-format=json");
    }
    let out = cmd.args(["-o", "/dev/null", file]).output()?;
    Ok(String::from_utf8_lossy(&out.stderr).into_owned())
}

/// A list of strings as Python prints it.
fn repr(items: &[String]) -> String {
    let quoted: Vec<String> = items
        .iter()
        .map(|s| {
            let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
            let body = s.replace('\\', "\\\\").replace('\n', "\\n");
            let body = if q == '\'' { body.replace('\'', "\\'") } else { body };
            format!("{q}{body}{q}")
        })
        .collect();
    format!("[{}]", quoted.join(", "))
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let file = args.file.to_string_lossy().into_owned();
    let src = std::fs::read(&args.file)?;
    let mut n = 0;
    for d in diagnostics(&check(&args.toolchain, &file, true)?) {
        for c in std::iter::once(&d).chain(&d.children) {
            let mut parts: Vec<(usize, usize, &str)> = c
                .spans
                .iter()
                .filter(|s| s.suggestion_applicability.as_deref() == Some("MachineApplicable"))
                .filter_map(|s| Some((s.byte_start, s.byte_end, s.suggested_replacement.as_deref()?)))
                .collect();
            if parts.is_empty() || d.level != "warning" {
                continue;
            }
            n += 1;
            let mut fixed = src.clone();
            parts.sort_by(|a, b| b.cmp(a));
            for &(a, b, t) in &parts {
                fixed.splice(a..b, t.bytes());
            }
            parts.reverse();
            let g = file.replacen(".rs", &format!("_fix{n}.rs"), 1);
            std::fs::write(&g, &fixed)?;
            let errs = check(&args.toolchain, &g, false)?;
            let first = errs.lines().find(|l| l.starts_with("error")).unwrap_or("compiles");
            let code = if d.code().is_empty() { "-" } else { d.code() };
            let before: Vec<String> = parts.iter().map(|&(a, b, _)| String::from_utf8_lossy(&src[a..b]).into_owned()).collect();
            let after: Vec<String> = parts.iter().map(|&(_, _, t)| t.to_owned()).collect();
            println!("  {code}: {} -> {}  => {first}", repr(&before), repr(&after));
        }
    }
    Ok(ExitCode::SUCCESS)
}

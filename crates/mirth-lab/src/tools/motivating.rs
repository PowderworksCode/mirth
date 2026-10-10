//! docs/motivating.md: the real rust-lang/rust bugs behind each property, each reproduced on a
//! toolchain from before its fix and one after.
//!
//! `motivating-run [issue…]` runs the reproductions in docs/motivating/bugs.json and saves the
//! outputs to out/<issue>/{before,after}.txt. `motivating-doc` writes motivating.md from
//! bugs.json, notes.json (what each run showed, the replays on mirth, the prose) and the outputs.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

use serde::Deserialize;

use mirth_lab::rustc::{Exit, run_command};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/motivating")
}

#[derive(Deserialize)]
struct Bug {
    issue: u64,
    title: String,
    fix_pr: u64,
    merged: String,
    before_toolchain: String,
    after_toolchain: String,
    properties: Vec<String>,
    how_it_violates: String,
    observe: String,
    files: Vec<File>,
    commands: String,
}

#[derive(Deserialize)]
struct File {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct Notes {
    header: String,
    /// Bugs only partly reproduced: what was seen (the rest are "reproduced").
    status: BTreeMap<String, String>,
    this_run: BTreeMap<String, String>,
    mirth: BTreeMap<String, String>,
    properties: Vec<Property>,
    footer: String,
}

#[derive(Deserialize)]
struct Property {
    key: String,
    title: String,
    why: String,
}

fn bugs() -> anyhow::Result<Vec<Bug>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(dir().join("bugs.json"))?)?)
}

#[derive(clap::Args, Debug)]
pub struct RunArgs {
    /// Only these issues.
    issues: Vec<u64>,
}

pub fn run(args: RunArgs) -> anyhow::Result<ExitCode> {
    for bug in bugs()? {
        if !args.issues.is_empty() && !args.issues.contains(&bug.issue) {
            continue;
        }
        let out = dir().join("out").join(bug.issue.to_string());
        std::fs::create_dir_all(&out)?;
        for (side, toolchain) in [("before", &bug.before_toolchain), ("after", &bug.after_toolchain)] {
            let installed = Command::new("rustup").args(["toolchain", "install", toolchain, "--profile", "minimal"]).output()?;
            if !installed.status.success() {
                std::fs::write(out.join(format!("{side}.txt")), format!("could not install {toolchain}\n"))?;
                continue;
            }
            let d = tempfile::tempdir()?;
            for f in &bug.files {
                let path = d.path().join(&f.path);
                std::fs::create_dir_all(path.parent().unwrap())?;
                std::fs::write(&path, &f.content)?;
            }
            let mut cmd = Command::new("bash");
            cmd.arg("-c")
                .arg(bug.commands.replace("TOOLCHAIN", toolchain))
                .current_dir(d.path())
                .env("RUSTC_WRAPPER", "")
                .env("CARGO_TERM_COLOR", "never");
            let done = run_command(cmd, Duration::from_secs(900))?;
            let text = match done.exit {
                Exit::Timeout => format!("$ toolchain {toolchain}\ntimed out\n"),
                Exit::Code(c) => format!("$ toolchain {toolchain}\n{}\n--- stderr ---\n{}\nexit {c}\n", done.stdout_text(), done.stderr_text()),
                Exit::Signal(s) => format!("$ toolchain {toolchain}\n{}\n--- stderr ---\n{}\nexit -{s}\n", done.stdout_text(), done.stderr_text()),
            };
            std::fs::write(out.join(format!("{side}.txt")), text)?;
        }
        println!("#{}: done", bug.issue);
    }
    Ok(ExitCode::SUCCESS)
}

#[derive(clap::Args, Debug)]
pub struct DocArgs {}

pub fn doc(_: DocArgs) -> anyhow::Result<ExitCode> {
    let bugs = bugs()?;
    let notes: Notes = serde_json::from_str(&std::fs::read_to_string(dir().join("notes.json"))?)?;
    let status = |b: &Bug| notes.status.get(&b.issue.to_string()).map_or("reproduced", String::as_str);
    let mut out: Vec<String> = vec![notes.header.clone()];
    for p in &notes.properties {
        let rows: Vec<&Bug> = bugs.iter().filter(|b| b.properties.contains(&p.key)).collect();
        out.push(format!("## {}: {}\n\n{}\n", p.key, p.title, p.why));
        out.push("| issue | fixed by | merged | before → after | status | mirth, with the fix reverted |\n|---|---|---|---|---|---|".into());
        for b in &rows {
            let title: String = b.title.split(" (").next().unwrap_or("").chars().take(80).collect();
            out.push(format!(
                "| [#{0}](https://github.com/rust-lang/rust/issues/{0}) {title} | [#{1}](https://github.com/rust-lang/rust/pull/{1}) | {2} | `{3}` → `{4}` | {5} | {6} |",
                b.issue,
                b.fix_pr,
                b.merged,
                b.before_toolchain,
                b.after_toolchain,
                status(b).split(':').next().unwrap_or(""),
                notes.mirth.get(&b.issue.to_string()).map_or("—", String::as_str)
            ));
        }
        out.push(String::new());
        for b in &rows {
            out.push(format!("**#{}.** {}\n", b.issue, b.how_it_violates));
            if status(b) != "reproduced" {
                out.push(format!("*Status:* {}.\n", status(b)));
            }
            let seen = notes.this_run.get(&b.issue.to_string()).map_or("", String::as_str);
            let mut line = String::new();
            let _ = write!(line, "*This run:* {seen}. Outputs: [`before`](motivating/out/{0}/before.txt), [`after`](motivating/out/{0}/after.txt).\n", b.issue);
            out.push(line);
            out.push(format!("*Expected, from the issue and the research:* {}\n", b.observe));
        }
    }
    out.push(notes.footer.clone());
    let text = out.join("\n");
    std::fs::write(dir().join("../motivating.md"), &text)?;
    println!("written {}", out.iter().map(|x| x.chars().count()).sum::<usize>());
    Ok(ExitCode::SUCCESS)
}

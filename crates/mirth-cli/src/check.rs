//! Properties of a recorded build, checked across all of its processes.
//!
//! - **P1** an `.rmeta` reaches its final path only by renaming the file the
//!   encoder wrote;
//! - **P2** no process opens an `.rmeta` before its writer renamed it into
//!   place;
//! - **P4** encoding reads no environment variable, clock or random state,
//!   except what the allow list names, each with a reason;
//! - **P7** no temporary file or directory is left in the target directory.

use std::path::{Path, PathBuf};

use crate::model::{Logged, Process, Record};
use crate::normalize::Normalize;
use crate::report::{Role, role, short};

pub struct Violation {
    pub property: &'static str,
    pub process: String,
    pub detail: String,
}

/// What encoding may read, with the reason it may: `<pattern> -- <reason>`
/// per line, where `*` in the pattern matches any run of characters.
pub struct Allow {
    entries: Vec<(String, String)>,
}

impl Allow {
    pub fn parse(text: &str) -> Result<Allow, String> {
        let mut entries = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (pattern, reason) = line
                .split_once(" -- ")
                .ok_or_else(|| format!("no reason given: {line}"))?;
            entries.push((pattern.trim().to_owned(), reason.trim().to_owned()));
        }
        Ok(Allow { entries })
    }

    fn allows(&self, what: &str) -> bool {
        self.entries.iter().any(|(pattern, _)| glob(pattern, what))
    }
}

fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let Some(rest) = text.strip_prefix(parts[0]) else {
        return false;
    };
    let mut rest = rest;
    for (n, part) in parts.iter().enumerate().skip(1) {
        if n == parts.len() - 1 {
            return rest.ends_with(part);
        }
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.is_empty()
}

struct Files<'a> {
    process: &'a Process,
    encoded: Vec<(&'a Logged, String)>,
    renamed: Vec<(&'a Logged, String, String)>,
    opened: Vec<(&'a Logged, String)>,
}

fn files<'a>(record: &'a Record, process: &'a Process) -> Files<'a> {
    let mut files = Files {
        process,
        encoded: Vec::new(),
        renamed: Vec::new(),
        opened: Vec::new(),
    };
    for logged in &process.logged {
        let Some(site) = record.site(logged.site) else {
            continue;
        };
        if role(site) != Role::File {
            continue;
        }
        let first = logged.arguments.first().cloned().unwrap_or_default();
        if site.target.contains("FileEncoder") {
            files.encoded.push((logged, first));
        } else if site.target == "std::fs::rename" {
            let second = logged.arguments.get(1).cloned().unwrap_or_default();
            files.renamed.push((logged, first, second));
        } else if site.target == "std::fs::File::open" {
            files.opened.push((logged, first));
        }
    }
    files
}

/// Where a process that emits metadata publishes it.
fn final_rmeta(process: &Process) -> Option<PathBuf> {
    if !process
        .flags("--emit")
        .iter()
        .any(|emit| emit.split(',').any(|it| it == "metadata"))
    {
        return None;
    }
    let out = process.flag("--out-dir")?;
    let extra = process
        .flags("-C")
        .into_iter()
        .find_map(|it| it.strip_prefix("extra-filename="))
        .unwrap_or("");
    Some(Path::new(out).join(format!("lib{}{extra}.rmeta", process.crate_name())))
}

pub fn check(record: &Record, normalize: &Normalize, allow: &Allow) -> Vec<Violation> {
    let mut violations = Vec::new();
    let all: Vec<Files> = record
        .processes
        .iter()
        .map(|process| files(record, process))
        .collect();

    for files in &all {
        let process = files.process;
        let label = crate::report::Report::label(process);
        let Some(target) = final_rmeta(process) else {
            continue;
        };
        if !process.exited {
            continue;
        }
        let target = target.to_string_lossy().into_owned();
        for (_, path) in &files.encoded {
            if *path == target {
                violations.push(Violation {
                    property: "P1",
                    process: label.clone(),
                    detail: format!("encoded straight to {}", normalize.path(path)),
                });
            }
        }
        let published = files.renamed.iter().any(|(_, from, to)| {
            *to == target && files.encoded.iter().any(|(_, path)| path == from)
        });
        if !published {
            violations.push(Violation {
                property: "P1",
                process: label.clone(),
                detail: format!(
                    "{} was not published by renaming the encoded file",
                    normalize.path(&target)
                ),
            });
        }
    }

    for reader in &all {
        for (opened, path) in &reader.opened {
            if !path.ends_with(".rmeta") {
                continue;
            }
            for writer in &all {
                for (renamed, _, to) in &writer.renamed {
                    if to == path && renamed.ns > opened.ns {
                        violations.push(Violation {
                            property: "P2",
                            process: crate::report::Report::label(reader.process),
                            detail: format!(
                                "opened {} before {} renamed it into place",
                                normalize.path(path),
                                writer.process.crate_name()
                            ),
                        });
                    }
                }
            }
        }
    }

    for process in &record.processes {
        for counted in &process.counted {
            let (Some(site), Some(frame)) =
                (record.site(counted.site), record.site(counted.frame.site))
            else {
                continue;
            };
            if role(site) != Role::State || site.kind == "touch" || !frame.caller.contains("encode")
            {
                continue;
            }
            let what = if counted.arguments.is_empty() {
                site.target.clone()
            } else {
                format!("{}({})", site.target, counted.arguments.join(", "))
            };
            if !allow.allows(&what) {
                violations.push(Violation {
                    property: "P4",
                    process: crate::report::Report::label(process),
                    detail: format!(
                        "{what} read by {} while encoding, in {}",
                        short(&site.caller),
                        short(&frame.caller)
                    ),
                });
            }
        }
    }
    violations
}

/// P7: temporary files and directories left in a target directory.
pub fn leftovers(target: &Path, normalize: &Normalize) -> Vec<Violation> {
    let mut found = Vec::new();
    let mut pending = vec![target.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let temporary = (name.starts_with("rmeta") && !name.contains('.'))
                || name.ends_with(".tmp")
                || name.starts_with(".tmp");
            if temporary {
                found.push(Violation {
                    property: "P7",
                    process: "-".to_owned(),
                    detail: format!("left behind: {}", normalize.path(&path.to_string_lossy())),
                });
            } else if entry.file_type().is_ok_and(|it| it.is_dir()) {
                pending.push(path);
            }
        }
    }
    found.sort_by(|a, b| a.detail.cmp(&b.detail));
    found
}

#[cfg(test)]
mod tests {
    use super::glob;

    #[test]
    fn globs() {
        assert!(glob(
            "std::env::var(CARGO_*)",
            "std::env::var(CARGO_PKG_NAME)"
        ));
        assert!(!glob("std::env::var(CARGO_*)", "std::env::var(HOME)"));
        assert!(glob("*", "x"));
    }
}

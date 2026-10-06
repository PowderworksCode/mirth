//! A recorded build: the site tables the instrumented compiler was built
//! with, and the log each process wrote.

use std::collections::HashMap;
use std::path::Path;

pub struct Site {
    pub kind: String,
    pub caller: String,
    pub target: String,
    pub snippet: String,
}

#[derive(Clone)]
pub struct Frame {
    pub site: u64,
    pub arguments: Vec<String>,
}

pub struct Logged {
    pub ns: u128,
    pub site: u64,
    pub frame: Frame,
    pub arguments: Vec<String>,
}

pub struct Counted {
    pub site: u64,
    pub frame: Frame,
    pub count: u64,
    pub arguments: Vec<String>,
}

pub struct Process {
    pub start: u128,
    pub arguments: Vec<String>,
    pub logged: Vec<Logged>,
    pub counted: Vec<Counted>,
    /// Whether the process reached its exit. Counts are written at exit, so a
    /// process that crashed has none.
    pub exited: bool,
}

impl Process {
    /// The value after `flag`, as `--flag value` or `--flag=value`.
    pub fn flag(&self, flag: &str) -> Option<&str> {
        let prefix = format!("{flag}=");
        self.arguments
            .iter()
            .enumerate()
            .find_map(|(at, argument)| {
                if argument == flag {
                    self.arguments.get(at + 1).map(String::as_str)
                } else {
                    argument.strip_prefix(&prefix)
                }
            })
    }

    pub fn flags(&self, flag: &str) -> Vec<&str> {
        let prefix = format!("{flag}=");
        let mut found = Vec::new();
        for (at, argument) in self.arguments.iter().enumerate() {
            if argument == flag {
                if let Some(value) = self.arguments.get(at + 1) {
                    found.push(value.as_str());
                }
            } else if let Some(value) = argument.strip_prefix(&prefix) {
                found.push(value);
            }
        }
        found
    }

    pub fn crate_name(&self) -> &str {
        self.flag("--crate-name").unwrap_or("?")
    }
}

pub struct Record {
    pub sites: HashMap<u64, Site>,
    pub processes: Vec<Process>,
}

impl Record {
    pub fn load(sites: &Path, logs: &Path) -> Result<Record, String> {
        let mut table = HashMap::new();
        for entry in read_dir(sites)? {
            if entry.extension().is_none_or(|it| it != "sites") {
                continue;
            }
            let text = read(&entry)?;
            for line in text.lines() {
                let fields: Vec<&str> = line.split('\t').collect();
                if fields.len() < 8 {
                    return Err(format!("{}: a short line: {line}", entry.display()));
                }
                table.insert(
                    number(fields[0])?,
                    Site {
                        kind: fields[1].to_owned(),
                        caller: fields[4].to_owned(),
                        target: fields[5].to_owned(),
                        snippet: fields[7].to_owned(),
                    },
                );
            }
        }

        let mut processes = Vec::new();
        for entry in read_dir(logs)? {
            processes.push(
                process(&read(&entry)?).map_err(|error| format!("{}: {error}", entry.display()))?,
            );
        }
        processes.sort_by_key(|process| process.start);
        Ok(Record {
            sites: table,
            processes,
        })
    }

    pub fn site(&self, id: u64) -> Option<&Site> {
        self.sites.get(&id)
    }
}

fn read_dir(directory: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|it| it.path()))
        .collect();
    paths.sort();
    Ok(paths)
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("not a number: {text:?}"))
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn frame(site: &str, arguments: &str) -> Result<Frame, String> {
    let arguments = unescape(arguments);
    Ok(Frame {
        site: number(site)?,
        arguments: if arguments.is_empty() {
            Vec::new()
        } else {
            arguments.split('\u{1f}').map(str::to_owned).collect()
        },
    })
}

fn process(text: &str) -> Result<Process, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("an empty log")?.split('\t').collect();
    if header.first() != Some(&"P") || header.len() < 3 {
        return Err("no process line first".to_owned());
    }
    let mut process = Process {
        start: number(header[2])?,
        arguments: header[3..].iter().map(|it| unescape(it)).collect(),
        logged: Vec::new(),
        counted: Vec::new(),
        exited: false,
    };
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.first() {
            Some(&"L") if fields.len() >= 7 => process.logged.push(Logged {
                ns: number(fields[1])?,
                site: number(fields[3])?,
                frame: frame(fields[4], fields[6])?,
                arguments: fields[7..].iter().map(|it| unescape(it)).collect(),
            }),
            Some(&"C") if fields.len() >= 6 => {
                process.exited = true;
                process.counted.push(Counted {
                    site: number(fields[1])?,
                    frame: frame(fields[2], fields[4])?,
                    count: number(fields[5])?,
                    arguments: fields[6..].iter().map(|it| unescape(it)).collect(),
                });
            }
            Some(&"X") => process.exited = true,
            _ => return Err(format!("an unreadable line: {line}")),
        }
    }
    Ok(process)
}

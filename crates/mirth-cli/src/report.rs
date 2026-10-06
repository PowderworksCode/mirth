//! A record as lists to read, review and bless: one section per process.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

use crate::model::{Frame, Process, Record, Site};
use crate::normalize::Normalize;

/// What a site is, for the report.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    File,
    Encode,
    Read,
    Register,
    Track,
    State,
    Other,
}

pub fn role(site: &Site) -> Role {
    let target = site.target.as_str();
    if site.kind == "touch" {
        return Role::State;
    }
    if target.starts_with("std::fs::") || target.contains("FileEncoder") {
        return Role::File;
    }
    if target.starts_with("std::env::")
        || target.starts_with("std::time::")
        || target.contains("RandomState::new")
        || target.starts_with("std::collections::HashMap::")
        || target.starts_with("std::collections::HashSet::")
    {
        return Role::State;
    }
    if target.contains("TableBuilder") {
        return Role::Encode;
    }
    if target.contains("LazyTable") || target.contains("::Lazy") {
        return Role::Read;
    }
    if target.ends_with("set_crate_data") {
        return Role::Register;
    }
    if target.ends_with("TyCtxtEnsureOk<'tcx>>::crate_hash") {
        return Role::Track;
    }
    Role::Other
}

/// The last two segments of a path: enough to name a function in a list.
pub fn short(path: &str) -> String {
    let parts: Vec<&str> = path.split("::").collect();
    parts[parts.len().saturating_sub(2)..].join("::")
}

/// The short name of a file operation: `rename`, `open`, `create`, ….
fn operation(target: &str) -> String {
    if target.contains("FileEncoder") && target.ends_with("::finish") {
        return "finished".to_owned();
    }
    if target.contains("FileEncoder") {
        return "encode-to".to_owned();
    }
    target.rsplit("::").next().unwrap_or(target).to_owned()
}

pub struct Report<'a> {
    pub record: &'a Record,
    pub normalize: &'a Normalize,
}

/// The table a `record!` site writes, from its source: `self.tables.fn_sig`.
pub fn table_of(snippet: &str) -> Option<&str> {
    let after = &snippet[snippet.find("tables.")? + "tables.".len()..];
    let end = after
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    Some(&after[..end])
}

impl Report<'_> {
    /// The entries each recorded crate wrote, by table: the indices
    /// `TableBuilder::set*` was called with.
    pub fn written(&self) -> HashMap<(String, String), BTreeSet<String>> {
        let mut written: HashMap<(String, String), BTreeSet<String>> = HashMap::new();
        for process in &self.record.processes {
            for counted in &process.counted {
                let Some(site) = self.record.site(counted.site) else {
                    continue;
                };
                if role(site) != Role::Encode {
                    continue;
                }
                let (Some(table), Some(index)) =
                    (table_of(&site.snippet), counted.arguments.first())
                else {
                    continue;
                };
                written
                    .entry((process.crate_name().to_owned(), table.to_owned()))
                    .or_default()
                    .insert(index.clone());
            }
        }
        written
    }

    fn frame_name(&self, frame: &Frame) -> String {
        match self.record.site(frame.site) {
            Some(site) => short(&site.caller),
            None if frame.site == 0 => "-".to_owned(),
            None => format!("frame {}", frame.site),
        }
    }

    /// Whether this record has the process that wrote `krate`'s metadata.
    fn writes(&self, krate: &str) -> bool {
        self.record.processes.iter().any(|process| {
            process.crate_name() == krate
                && process
                    .flags("--emit")
                    .iter()
                    .any(|it| it.contains("metadata"))
        })
    }

    /// The crates this process loaded from the sysroot, by the files it
    /// opened: their reads are summed together in the report.
    fn sysroot_crates(&self, process: &Process) -> std::collections::HashSet<String> {
        let mut found = std::collections::HashSet::new();
        for logged in &process.logged {
            let Some(site) = self.record.site(logged.site) else {
                continue;
            };
            if site.target != "std::fs::File::open" {
                continue;
            }
            let Some(path) = logged.arguments.first() else {
                continue;
            };
            if !self.normalize.path(path).starts_with("<sysroot>") {
                continue;
            }
            let name = std::path::Path::new(path)
                .file_stem()
                .and_then(|it| it.to_str())
                .and_then(|it| it.strip_prefix("lib"))
                .map(|it| it.split('-').next().unwrap_or(it).to_owned());
            found.extend(name);
        }
        found
    }

    /// Crate numbers as this process assigned them. Crates from the sysroot
    /// are all called `<sysroot>`.
    pub fn crates(&self, process: &Process) -> HashMap<String, String> {
        let sysroot = self.sysroot_crates(process);
        let mut crates = HashMap::new();
        crates.insert("0".to_owned(), process.crate_name().to_owned());
        for counted in &process.counted {
            let Some(site) = self.record.site(counted.site) else {
                continue;
            };
            if role(site) != Role::Register {
                continue;
            }
            if let (Some(number), Some(name)) =
                (counted.arguments.first(), counted.frame.arguments.first())
            {
                let name = name.trim_matches('"');
                let name = if sysroot.contains(name) {
                    "<sysroot>"
                } else {
                    name
                };
                crates.insert(number.clone(), name.to_owned());
            }
        }
        crates
    }

    pub fn label(process: &Process) -> String {
        let types = process.flags("--crate-type").join(",");
        let emit = process.flags("--emit").join(",");
        let mut label = process.crate_name().to_owned();
        if !types.is_empty() {
            label.push_str(&format!(" ({types})"));
        }
        if !emit.is_empty() {
            label.push_str(&format!(" emit {emit}"));
        }
        label
    }

    pub fn write(&self) -> String {
        let mut sections: Vec<(String, String)> = self
            .record
            .processes
            .iter()
            .map(|process| (Self::label(process), self.process(process)))
            .collect();
        sections.sort();
        let mut text = String::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        for (label, body) in sections {
            let n = seen.entry(label.clone()).or_default();
            *n += 1;
            let label = if *n > 1 {
                format!("{label} #{n}")
            } else {
                label
            };
            let _ = writeln!(text, "== {label} ==");
            text.push_str(&body);
            text.push('\n');
        }
        text
    }

    fn process(&self, process: &Process) -> String {
        let crates = self.crates(process);
        // A query key is a `CrateNum` alone; a `DefId`, captured as
        // `index:krate` (its field order on little-endian targets); or a
        // `(CrateNum, DefId)`, which reads the first crate's metadata.
        let crate_of = |key: &str| -> String {
            let parts: Vec<&str> = key.split(':').collect();
            let number = match parts.as_slice() {
                [_, krate] => krate,
                [krate, ..] => krate,
                [] => key,
            };
            crates
                .get(number)
                .cloned()
                .unwrap_or_else(|| format!("crate {number}"))
        };
        let mut text = String::new();

        if !process.exited {
            text.push_str("did not exit: stopped or crashed\n");
        }

        let mut files = String::new();
        for logged in &process.logged {
            let Some(site) = self.record.site(logged.site) else {
                continue;
            };
            if role(site) != Role::File {
                continue;
            }
            let arguments: Vec<String> = logged
                .arguments
                .iter()
                .map(|it| self.normalize.path(it))
                .collect();
            let _ = writeln!(
                files,
                "  {:<14} {:<40} in {}",
                operation(&site.target),
                arguments.join(" -> "),
                self.frame_name(&logged.frame)
            );
        }
        if !files.is_empty() {
            text.push_str("files\n");
            text.push_str(&files);
        }

        let mut encodes: BTreeMap<String, (BTreeSet<String>, u64)> = BTreeMap::new();
        let mut reads: BTreeMap<(String, String), (BTreeSet<String>, u64)> = BTreeMap::new();
        let mut tracked: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        let mut untracked: BTreeMap<(String, String), u64> = BTreeMap::new();
        let mut state: BTreeMap<(String, String, String), u64> = BTreeMap::new();
        for counted in &process.counted {
            let Some(site) = self.record.site(counted.site) else {
                continue;
            };
            match role(site) {
                Role::Encode => {
                    let entry = encodes.entry(site.snippet.clone()).or_default();
                    entry.0.extend(counted.arguments.first().cloned());
                    entry.1 += counted.count;
                }
                Role::Track => {
                    if let Some(frame) = self.record.site(counted.frame.site)
                        && frame.caller.contains("provide_extern")
                        && let Some(key) = counted.frame.arguments.first()
                    {
                        let query = frame.caller.rsplit("::").next().unwrap_or("?");
                        tracked
                            .entry((crate_of(key), query.to_owned()))
                            .or_default()
                            .insert(key.clone());
                    }
                }
                Role::Read => {
                    let frame = self.record.site(counted.frame.site);
                    match frame {
                        Some(frame) if frame.caller.contains("provide_extern") => {
                            let query = frame.caller.rsplit("::").next().unwrap_or("?");
                            let key = counted.frame.arguments.first();
                            let krate = key.map_or_else(|| "?".to_owned(), |key| crate_of(key));
                            let entry = reads.entry((krate, query.to_owned())).or_default();
                            entry.0.extend(key.cloned());
                            entry.1 += counted.count;
                        }
                        _ => {
                            *untracked
                                .entry((self.frame_name(&counted.frame), short(&site.caller)))
                                .or_default() += counted.count;
                        }
                    }
                }
                Role::State => {
                    let what = if counted.arguments.is_empty() {
                        site.target.clone()
                    } else {
                        format!("{}({})", site.target, counted.arguments.join(", "))
                    };
                    *state
                        .entry((what, short(&site.caller), self.frame_name(&counted.frame)))
                        .or_default() += counted.count;
                }
                _ => {}
            }
        }

        if !encodes.is_empty() {
            text.push_str("encoded\n");
            for (snippet, (items, count)) in &encodes {
                let _ = writeln!(text, "  {count:>6} {:>5} items  {snippet}", items.len());
            }
        }
        if !reads.is_empty() {
            let written = self.written();
            let tables: std::collections::HashSet<&str> =
                written.keys().map(|(_, table)| table.as_str()).collect();
            text.push_str("read through queries: reads, items, items whose query recorded a dependency on the crate, entries the writer wrote\n");
            for ((krate, query), (keys, count)) in &reads {
                let found = match written.get(&(krate.clone(), query.clone())) {
                    Some(entries) => {
                        let hits = keys
                            .iter()
                            .filter(|key| {
                                key.split_once(':')
                                    .is_some_and(|(index, _)| entries.contains(index))
                            })
                            .count();
                        format!("{hits} written")
                    }
                    None if tables.contains(query.as_str()) && self.writes(krate) => {
                        "none written".to_owned()
                    }
                    None => String::new(),
                };
                let kept = tracked
                    .get(&(krate.clone(), query.clone()))
                    .map_or(0, |it| it.intersection(keys).count());
                let _ = writeln!(
                    text,
                    "  {count:>6} {:>5} items {kept:>5} tracked {found:>12}  {krate:<18} {query}",
                    keys.len()
                );
            }
        }
        if !untracked.is_empty() {
            text.push_str("read outside a query\n");
            for ((frame, caller), count) in &untracked {
                let _ = writeln!(text, "  {count:>6}  {caller:<50} in {frame}");
            }
        }
        if !state.is_empty() {
            text.push_str("state\n");
            for ((what, caller, frame), count) in &state {
                let _ = writeln!(text, "  {count:>6}  {what}  by {caller}  in {frame}");
            }
        }
        text
    }
}

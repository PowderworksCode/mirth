//! Compiler outputs in a form two builds can be compared in: archives taken apart, session
//! suffixes in member names and link metadata removed, everything hashed.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::bytes::Regex;
use sha2::{Digest, Sha256};

/// `.<7 chars>.rcgu.o`: the per-session suffix of codegen-unit objects.
static SESSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.[0-9a-z]{7}(\.rcgu\.(?:o|dwo))").unwrap());

pub fn sha256(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The members of a Unix ar archive (GNU format), names with session suffixes removed.
pub fn ar_members(data: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut members = BTreeMap::new();
    if !data.starts_with(b"!<arch>\n") {
        members.insert("<not an archive>".into(), data.to_vec());
        return members;
    }
    let (mut names, mut pos): (&[u8], usize) = (b"", 8);
    while pos + 60 <= data.len() {
        let header = &data[pos..pos + 60];
        let raw_name = trim_end(&header[..16]);
        let size: usize = std::str::from_utf8(&header[48..58]).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        let end = (pos + 60 + size).min(data.len());
        let body = &data[pos + 60..end];
        pos += 60 + size + (size & 1);
        if raw_name == b"//" {
            names = body;
            continue;
        }
        if raw_name == b"/" || raw_name == b"/SYM64/" {
            continue;
        }
        let mut name: Vec<u8> = raw_name.to_vec();
        if name.len() > 1 && name[0] == b'/' && name[1..].iter().all(u8::is_ascii_digit) {
            let offset: usize = std::str::from_utf8(&name[1..]).unwrap().parse().unwrap_or(0);
            let rest = names.get(offset..).unwrap_or(b"");
            let len = rest.windows(2).position(|w| w == b"/\n").unwrap_or(rest.len());
            name = rest[..len].to_vec();
        }
        while name.last() == Some(&b'/') {
            name.pop();
        }
        let name = SESSION.replace_all(&name, &b"$1"[..]);
        members.insert(String::from_utf8_lossy(&name).into_owned(), body.to_vec());
    }
    members
}

fn trim_end(b: &[u8]) -> &[u8] {
    let n = b.iter().rposition(|c| !c.is_ascii_whitespace()).map_or(0, |i| i + 1);
    &b[..n]
}

/// An rlib as {member: digest}, session suffixes removed from names and contents.
pub fn normalized_rlib(path: &Path) -> BTreeMap<String, String> {
    let data = std::fs::read(path).unwrap_or_default();
    ar_members(&data)
        .into_iter()
        .map(|(name, body)| (name, sha256(&SESSION.replace_all(&body, &b"$1"[..]))))
        .collect()
}

/// Every file in `dir` as {name: digest}; rlibs are normalized member by member.
pub fn digest_dir(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let digest = if p.extension().is_some_and(|x| x == "rlib") {
            serde_json::to_string(&normalized_rlib(&p)).unwrap_or_default()
        } else {
            sha256(&std::fs::read(&p).unwrap_or_default())
        };
        out.insert(name, digest);
    }
    out
}

/// What a Cargo build produced for packages built from a path, from the JSON messages of
/// `cargo build --message-format=json-render-diagnostics`: every .rmeta and executable by
/// digest, every .rlib member by member, every rendered diagnostic counted per crate. Paths are
/// relative to the target directory.
#[derive(Default, Debug, PartialEq, serde::Serialize)]
pub struct Collected {
    pub rmeta: BTreeMap<String, String>,
    pub rlib: BTreeMap<String, BTreeMap<String, String>>,
    pub exe: BTreeMap<String, String>,
    pub diag: BTreeMap<(String, String), usize>,
}

pub fn collect(stdout: &str, target: &Path) -> Collected {
    let mut found = Collected::default();
    let rel = |f: &str| Path::new(f).strip_prefix(target).map_or(f.to_owned(), |r| r.display().to_string());
    for line in stdout.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if !msg["package_id"].as_str().unwrap_or("").contains("path+file") {
            continue;
        }
        match msg["reason"].as_str() {
            Some("compiler-message") => {
                let m = &msg["message"];
                let text = m["rendered"].as_str().filter(|t| !t.is_empty()).or(m["message"].as_str()).unwrap_or("");
                let krate = msg["target"]["name"].as_str().unwrap_or("").to_owned();
                *found.diag.entry((krate, text.to_owned())).or_default() += 1;
            }
            Some("compiler-artifact") => {
                for f in msg["filenames"].as_array().into_iter().flatten().filter_map(|f| f.as_str()) {
                    if f.ends_with(".rmeta") {
                        found.rmeta.insert(rel(f), sha256(&std::fs::read(f).unwrap_or_default()));
                    } else if f.ends_with(".rlib") {
                        found.rlib.insert(rel(f), normalized_rlib(Path::new(f)));
                    }
                }
                if let Some(f) = msg["executable"].as_str() {
                    let data = std::fs::read(f).unwrap_or_default();
                    found.exe.insert(rel(f), sha256(&SESSION.replace_all(&data, &b"$1"[..])));
                }
            }
            _ => {}
        }
    }
    found
}

/// {kind: [what differs]} for the kinds that differ between two collections.
pub fn compare(a: &Collected, b: &Collected) -> BTreeMap<&'static str, Vec<String>> {
    fn keys<'a, V: PartialEq>(a: &'a BTreeMap<String, V>, b: &'a BTreeMap<String, V>) -> Vec<String> {
        let all: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
        all.into_iter().filter(|k| a.get(*k) != b.get(*k)).cloned().collect()
    }
    let mut out = BTreeMap::new();
    for (kind, x, y) in [("rmeta", &a.rmeta, &b.rmeta), ("exe", &a.exe, &b.exe)] {
        let diff = keys(x, y);
        if !diff.is_empty() {
            out.insert(kind, diff);
        }
    }
    let empty = BTreeMap::new();
    let rlibs: Vec<String> = keys(&a.rlib, &b.rlib)
        .into_iter()
        .map(|rel| {
            let members = keys(a.rlib.get(&rel).unwrap_or(&empty), b.rlib.get(&rel).unwrap_or(&empty));
            let more = if members.len() > 5 { " …" } else { "" };
            format!("{rel}: {}{more}", members[..members.len().min(5)].join(", "))
        })
        .collect();
    if !rlibs.is_empty() {
        out.insert("rlib", rlibs);
    }
    if a.diag != b.diag {
        let only = |x: &BTreeMap<(String, String), usize>, y: &BTreeMap<(String, String), usize>, which: &str| -> Vec<String> {
            x.iter()
                .filter(|(k, n)| **n > y.get(*k).copied().unwrap_or(0))
                .map(|((c, t), _)| format!("{c} only in the {which}: {:?}", t.chars().take(200).collect::<String>()))
                .collect()
        };
        let mut d = only(&a.diag, &b.diag, "first");
        d.extend(only(&b.diag, &a.diag, "second"));
        out.insert("diag", d);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_suffixes_removed() {
        let mut ar = b"!<arch>\n".to_vec();
        let body = b"refers to t.abc1234.rcgu.o";
        ar.extend(format!("{:<16}{:<32}{:<10}`\n", "t.abc1234.rcgu.o/", "", body.len()).as_bytes());
        ar.extend(body);
        let m = ar_members(&ar);
        assert!(m.contains_key("t.rcgu.o"), "{m:?}");
    }

    #[test]
    fn diagnostics_compared_by_count() {
        let line = |t: &str| format!(r#"{{"reason":"compiler-message","package_id":"path+file:///x#a@0.1.0","target":{{"name":"a"}},"message":{{"rendered":"{t}"}}}}"#);
        let a = collect(&format!("{}\n{}", line("w"), line("w")), Path::new("/t"));
        let b = collect(&line("w"), Path::new("/t"));
        assert_eq!(compare(&a, &b)["diag"], vec![r#"a only in the first: "w""#]);
        assert!(compare(&a, &a).is_empty());
    }
}

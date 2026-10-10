//! What a coverage-instrumented compiler (rustc/coverage.toml) writes: the site tables of its
//! build (`<crate>.sites`) and the `V <site>` lines each process run with MIRTH_OUT logs.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::ser::Formatter;

/// An instrumented function: a `cover` site.
pub struct Function {
    pub site: String,
    pub krate: String,
    pub path: String,
    pub span: String,
}

/// The files of `dir` with extension `ext`, not recursing.
pub fn files_with(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == ext)).collect()
}

fn read_lossy(path: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap_or_default()).into_owned()
}

/// Every `cover` site of the tables in `dir`.
pub fn functions(dir: &Path) -> Vec<Function> {
    let mut out = Vec::new();
    for table in files_with(dir, "sites") {
        for line in read_lossy(&table).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 7 && f[1] == "cover" {
                out.push(Function { site: f[0].into(), krate: f[3].into(), path: f[4].into(), span: f[6].into() });
            }
        }
    }
    out
}

/// The sites a log reached.
pub fn log_hits(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter_map(|l| l.strip_prefix("V\t"))
}

/// The sites the logs in `dir` reached, and how many logs there were.
pub fn hits(dir: &Path) -> (BTreeSet<String>, usize) {
    let logs = files_with(dir, "log");
    let mut out = BTreeSet::new();
    for log in &logs {
        out.extend(log_hits(&read_lossy(log)).map(str::to_owned));
    }
    (out, logs.len())
}

/// Python's json.dumps spacing (`, ` and `: `) and escaping (non-ASCII as `\\uXXXX`), on one
/// line or, with `indent`, as `json.dumps(…, indent=n)` writes it.
struct Python<'a> {
    indent: Option<&'a [u8]>,
    depth: usize,
    has_value: bool,
}

impl Python<'_> {
    fn newline<W: ?Sized + Write>(&self, w: &mut W) -> io::Result<()> {
        if let Some(indent) = self.indent {
            w.write_all(b"\n")?;
            for _ in 0..self.depth {
                w.write_all(indent)?;
            }
        }
        Ok(())
    }
    fn open<W: ?Sized + Write>(&mut self, w: &mut W, c: &[u8]) -> io::Result<()> {
        self.depth += 1;
        self.has_value = false;
        w.write_all(c)
    }
    fn close<W: ?Sized + Write>(&mut self, w: &mut W, c: &[u8]) -> io::Result<()> {
        self.depth -= 1;
        if self.has_value {
            self.newline(w)?;
        }
        self.has_value = true;
        w.write_all(c)
    }
    fn item<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        match (first, self.indent.is_some()) {
            (true, _) => {}
            (false, true) => w.write_all(b",")?,
            (false, false) => w.write_all(b", ")?,
        }
        self.newline(w)
    }
}

impl Formatter for Python<'_> {
    fn begin_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.open(w, b"[")
    }
    fn end_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.close(w, b"]")
    }
    fn begin_array_value<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.item(w, first)
    }
    fn end_array_value<W: ?Sized + Write>(&mut self, _: &mut W) -> io::Result<()> {
        self.has_value = true;
        Ok(())
    }
    fn begin_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.open(w, b"{")
    }
    fn end_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.close(w, b"}")
    }
    fn begin_object_key<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.item(w, first)
    }
    fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }
    fn end_object_value<W: ?Sized + Write>(&mut self, _: &mut W) -> io::Result<()> {
        self.has_value = true;
        Ok(())
    }
    fn write_string_fragment<W: ?Sized + Write>(&mut self, w: &mut W, fragment: &str) -> io::Result<()> {
        for c in fragment.chars() {
            if c.is_ascii() {
                w.write_all(&[c as u8])?;
            } else {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    write!(w, "\\u{unit:04x}")?;
                }
            }
        }
        Ok(())
    }
}

fn python_json(value: &impl serde::Serialize, indent: Option<&[u8]>) -> String {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, Python { indent, depth: 0, has_value: false });
    value.serialize(&mut ser).expect("serializable");
    String::from_utf8(buf).expect("utf-8")
}

/// One line of JSON as Python's `json.dumps(value)` writes it.
pub fn to_json_line(value: &impl serde::Serialize) -> String {
    python_json(value, None)
}

/// JSON as Python's `json.dumps(value, indent=n)` writes it.
pub fn to_json_indent(value: &impl serde::Serialize, n: usize) -> String {
    python_json(value, Some(&b"        "[..n]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_spacing() {
        let v = serde_json::json!({"a": [1, 2], "b": {}, "c": "é", "d": []});
        assert_eq!(to_json_line(&v), r#"{"a": [1, 2], "b": {}, "c": "\u00e9", "d": []}"#);
        assert_eq!(to_json_indent(&v, 1), "{\n \"a\": [\n  1,\n  2\n ],\n \"b\": {},\n \"c\": \"\\u00e9\",\n \"d\": []\n}");
        assert_eq!(to_json_indent(&serde_json::json!({"t": [0]}), 0), "{\n\"t\": [\n0\n]\n}");
    }
}

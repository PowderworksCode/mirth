//! rustc's UI tests as the checks use them: their `//@` headers (the first revision of a test
//! with revisions), and which of them can be compiled on their own on this host.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use walkdir::WalkDir;

/// What a test expects of the compiler, from its `//@ <kind>` header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    CheckPass,
    BuildPass,
    RunPass,
    CheckFail,
    BuildFail,
    RunFail,
}

impl Kind {
    fn parse(key: &str) -> Option<Kind> {
        Some(match key {
            "check-pass" => Kind::CheckPass,
            "build-pass" => Kind::BuildPass,
            "run-pass" => Kind::RunPass,
            "check-fail" => Kind::CheckFail,
            "build-fail" => Kind::BuildFail,
            "run-fail" => Kind::RunFail,
            _ => return None,
        })
    }

    /// Whether type checking is all the test asks for (metadata is enough to compile it).
    pub fn is_check(kind: Option<Kind>) -> bool {
        matches!(kind, None | Some(Kind::CheckPass) | Some(Kind::CheckFail))
    }
}

/// Every kind, and tests without a kind header.
pub const ALL: &[Option<Kind>] = &[
    Some(Kind::CheckPass),
    Some(Kind::BuildPass),
    Some(Kind::RunPass),
    Some(Kind::CheckFail),
    Some(Kind::BuildFail),
    Some(Kind::RunFail),
    None,
];
pub const RUNNABLE: &[Option<Kind>] = &[Some(Kind::RunPass), Some(Kind::RunFail)];

/// A UI test, as one of its revisions compiles it.
#[derive(Clone, Debug)]
pub struct Test {
    pub path: PathBuf,
    /// The path below the test root, as findings and known lists name it.
    pub rel: String,
    pub text: String,
    pub flags: Vec<String>,
    pub edition: Option<String>,
    pub kind: Option<Kind>,
    pub revision: Option<String>,
}

impl Test {
    pub fn edition(&self) -> &str {
        self.edition.as_deref().unwrap_or("2015")
    }

    pub fn file_name(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("test.rs")
    }
}

static REVISIONS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^//@\s*revisions:\s*(.*)$").unwrap());
static DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^//@(?:\[([\w,-]+)\])?\s*([a-z-]+)(?::\s*(.*))?$").unwrap());
/// Tests that need more than one file, another target, or a tool this host may lack.
static NOT_STANDALONE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^//@\s*(aux-build|aux-crate|aux-bin|aux-codegen-backend|proc-macro|add-minicore|needs-llvm-components|needs-sanitizer|needs-profiler|needs-rust-lld|needs-enzyme|ignore-x86_64|ignore-linux|ignore-unix|ignore-64bit|known-bug|rustc-env|unset-rustc-env)\b",
    )
    .unwrap()
});
static ONLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^//@\s*only-(\S+)").unwrap());
/// `only-<x>` directives this host satisfies.
const HOST: &[&str] = &["x86_64", "linux", "unix", "64bit", "elf", "gnu"];

/// The headers of a test's first revision.
pub fn headers(text: &str) -> (Vec<String>, Option<String>, Option<Kind>, Option<String>) {
    let revision =
        REVISIONS.captures(text).and_then(|c| c[1].split_whitespace().next().map(str::to_owned));
    let (mut flags, mut edition, mut kind) = (Vec::new(), None, None);
    for c in DIRECTIVE.captures_iter(text) {
        if let Some(only) = c.get(1)
            && !revision.as_deref().is_some_and(|r| only.as_str().split(',').any(|o| o == r))
        {
            continue;
        }
        let value = c.get(3).map_or("", |v| v.as_str().trim());
        match &c[2] {
            "compile-flags" => flags.extend(value.split_whitespace().map(str::to_owned)),
            "edition" => edition = value.split_whitespace().next().map(str::to_owned),
            key => {
                if let Some(k) = Kind::parse(key) {
                    kind = Some(k);
                }
            }
        }
    }
    if let Some(r) = &revision {
        flags.extend(["--cfg".to_owned(), r.clone()]);
    }
    (flags, edition, kind, revision)
}

/// Whether a test can be compiled alone on this host.
pub fn standalone(text: &str) -> bool {
    !NOT_STANDALONE.is_match(text)
        && ONLY.captures_iter(text).all(|c| HOST.iter().any(|h| c[1].starts_with(h)))
}

/// The standalone tests under `root` of the given kinds, minus those `skip` rejects.
pub fn tests(root: &Path, kinds: &[Option<Kind>], skip: impl Fn(&Test) -> bool) -> Vec<Test> {
    let mut out = Vec::new();
    let mut paths: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().is_some_and(|x| x == "rs")
                && !e.path().components().any(|c| c.as_os_str() == "auxiliary")
        })
        .map(|e| e.into_path())
        .collect();
    paths.sort();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if !standalone(&text) {
            continue;
        }
        let (flags, edition, kind, revision) = headers(&text);
        if !kinds.contains(&kind) {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
        let test = Test { path, rel, text, flags, edition, kind, revision };
        if !skip(&test) {
            out.push(test);
        }
    }
    out
}

/// Whether the test (or its flags) names one of these, as a substring of a flag.
pub fn flag_matches(test: &Test, re: &Regex) -> bool {
    test.flags.iter().any(|f| re.is_match(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_revision_headers() {
        let text = "//@ revisions: a b\n//@[a] compile-flags: -Zfoo\n//@[b] compile-flags: -Zbar\n//@ edition: 2021\n//@[a] check-pass\n";
        let (flags, edition, kind, rev) = headers(text);
        assert_eq!(flags, ["-Zfoo", "--cfg", "a"]);
        assert_eq!(edition.as_deref(), Some("2021"));
        assert_eq!(kind, Some(Kind::CheckPass));
        assert_eq!(rev.as_deref(), Some("a"));
    }

    #[test]
    fn host_only_directives() {
        assert!(standalone("//@ only-x86_64\n"));
        assert!(standalone("//@ only-linux\n"));
        assert!(!standalone("//@ only-aarch64\n"));
        assert!(!standalone("//@ aux-build: x.rs\n"));
    }
}

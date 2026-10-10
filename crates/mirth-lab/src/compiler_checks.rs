//! The patched compiler's own checks (docs/shadow-mode.md, docs/untracked-reads.md), turned on
//! for any compile: `RUSTC_VERIFY_REUSE` recomputes reused (with `all`, every cached) query
//! values at the end of the session and compares them, and `RUSTC_REPORT_UNTRACKED` reports
//! reads of untracked state inside reusable tasks. Both hook the dependency graph, so they act
//! only in incremental sessions: a compile with the checks on gets `-Cincremental`.
//!
//! An untracked read is known when its (what, file) pair is in rustc/untracked-known.tsv: every
//! pair the fuzzer, the replays and the flag walks have reported, each with a verdict in
//! docs/untracked-reads.md, and options known anywhere (file `*`). A new site of "source text"
//! is a note (the class has a verdict: a position or a wording computed from raw text). Anything
//! else is new.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::cargo;

/// The environment that turns the checks on; `all` also recomputes values computed this session.
pub fn env(all: bool) -> [(&'static str, &'static str); 2] {
    [("RUSTC_VERIFY_REUSE", if all { "all" } else { "1" }), ("RUSTC_REPORT_UNTRACKED", "1")]
}

/// The (what, file) pairs already reported and triaged, and the reuse reports
/// (rustc/reuse-known.txt).
pub struct Known(BTreeSet<(String, String)>, BTreeSet<String>);

impl Known {
    fn contains(&self, what: &str, file: &str) -> bool {
        self.0.contains(&(what.to_owned(), file.to_owned())) || self.0.contains(&(what.to_owned(), "*".to_owned()))
    }

    pub fn load() -> Known {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rustc/untracked-known.tsv");
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let reuse = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rustc/reuse-known.txt")).unwrap_or_default();
        let lines = |t: &str| t.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).map(str::to_owned).collect::<Vec<_>>();
        Known(
            lines(&text).iter().filter_map(|l| l.split_once('\t')).map(|(a, b)| (a.to_owned(), b.to_owned())).collect(),
            lines(&reuse).into_iter().collect(),
        )
    }
}

static READ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^rustc-untracked-read: (.*?), read at ([^ ,]+?)(?::\d+:\d+)?, while computing `([^`]+)`").unwrap());
static MODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^rustc-untracked-read: (printing mode `[^`]+`), inherited").unwrap());

/// What the checks reported in one compile's stderr.
#[derive(Default, Debug)]
pub struct Report {
    /// Stale reuse (`query <name>`, `metadata`, `codegen unit`, `allocation sharing ...`).
    pub reuse: Vec<String>,
    /// Untracked reads not in the known list: `<what> at <file> while computing <task>`.
    pub untracked: Vec<String>,
    /// New sites of a known class (source text).
    pub notes: Vec<String>,
}

impl Report {
    pub fn findings(&self) -> Vec<String> {
        self.reuse.iter().map(|r| format!("verify-reuse: {r}")).chain(self.untracked.iter().map(|u| format!("untracked: {u}"))).collect()
    }
}

pub fn read(stderr: &str, known: &Known) -> Report {
    let (mut untracked, mut notes) = (BTreeSet::new(), BTreeSet::new());
    for line in stderr.lines().filter(|l| l.starts_with("rustc-untracked-read:")) {
        if let Some(c) = READ.captures(line) {
            if !known.contains(&c[1], &c[2]) {
                let what = format!("{} at {} while computing {}", &c[1], &c[2], &c[3]);
                if &c[1] == "source text" { notes.insert(what) } else { untracked.insert(what) };
            }
        } else if let Some(c) = MODE.captures(line) {
            if !known.0.iter().any(|(w, _)| w == &c[1]) {
                untracked.insert(line.trim_start_matches("rustc-untracked-read: ").to_owned());
            }
        } else {
            untracked.insert(line.to_owned());
        }
    }
    let (known_reuse, reuse): (Vec<String>, Vec<String>) = cargo::reuse_checks(stderr).into_iter().partition(|r| known.1.contains(r));
    notes.extend(known_reuse.into_iter().map(|r| format!("known reuse report: {r}")));
    Report { reuse, untracked: untracked.into_iter().collect(), notes: notes.into_iter().collect() }
}

/// The stderr without the checks' lines (a reuse report's indented continuation lines too),
/// for comparing diagnostics.
pub fn strip(stderr: &str) -> String {
    let mut out = String::new();
    let mut in_report = false;
    for l in stderr.lines() {
        if l.starts_with("rustc-untracked-read:") || l.starts_with("rustc-verify-reuse") {
            in_report = true;
            continue;
        }
        if in_report && l.starts_with("  ") {
            continue;
        }
        in_report = false;
        out.push_str(l);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_and_new_reads() {
        let known = Known([("the untracked option jobs".to_owned(), "compiler/rustc_query_impl/src/execution.rs".to_owned())].into(), Default::default());
        let log = "rustc-untracked-read: the untracked option jobs, read at compiler/rustc_query_impl/src/execution.rs:295:24, while computing `crate_name`, whose result incremental compilation may reuse\n\
                   rustc-untracked-read: the untracked option no_leak_check, read at compiler/rustc_infer/src/infer/relate/higher_ranked.rs:95:47, while computing `typeck_root`, whose result incremental compilation may reuse\n\
                   rustc-verify-reuse: metadata differs at byte 5\n  allocation: memory, 0 bytes\nerror: x\n";
        let r = read(log, &known);
        assert_eq!(r.untracked, vec!["the untracked option no_leak_check at compiler/rustc_infer/src/infer/relate/higher_ranked.rs while computing typeck_root"]);
        assert_eq!(r.reuse, vec!["metadata"]);
        assert_eq!(strip(log), "error: x\n");
    }
}

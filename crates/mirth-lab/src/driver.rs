//! Running a check over many tests: a thread pool, one JSON line of results per test, findings
//! written to their own directories, and the frontier loop's options (pause at the first
//! finding, recheck only the tests with findings, leave known findings out).

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rayon::prelude::*;
use serde::Serialize;
use serde_json::Value;

use crate::uitest::Test;

/// The options every sweep over UI tests takes.
#[derive(clap::Args, Clone, Debug)]
pub struct Sweep {
    /// The rustc's test directory (`<rust>/tests/ui`).
    #[arg(long)]
    pub tests: PathBuf,
    /// Where results, findings and scratch files go.
    #[arg(long)]
    pub work: PathBuf,
    /// Only tests whose path contains this.
    #[arg(long)]
    pub only: Option<String>,
    /// A file of test paths to leave out (known findings).
    #[arg(long)]
    pub known: Option<PathBuf>,
    #[arg(long, default_value_t = 8)]
    pub jobs: usize,
    /// Stop starting new tests at the first finding, and exit 3 (the frontier loop).
    #[arg(long)]
    pub pause_on_finding: bool,
    /// Run only the tests that have findings under <work>/findings.
    #[arg(long)]
    pub recheck: bool,
    /// Compile with the patched compiler's own checks on (RUSTC_VERIFY_REUSE=all,
    /// RUSTC_REPORT_UNTRACKED; an incremental session), where the check supports it.
    #[arg(long)]
    pub compiler_checks: bool,
}

impl Sweep {
    pub fn scratch(&self) -> PathBuf {
        let s = self.work.join("scratch");
        let _ = fs::create_dir_all(&s);
        s
    }

    /// Filter `tests` by --only, --known and --recheck.
    pub fn select(&self, tests: Vec<Test>) -> Vec<Test> {
        let known: BTreeSet<String> = self
            .known
            .as_ref()
            .and_then(|k| fs::read_to_string(k).ok())
            .map(|t| t.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default();
        let wanted = self.recheck.then(|| findings_tests(&self.work));
        tests
            .into_iter()
            .filter(|t| {
                !known.contains(&t.rel)
                    && self.only.as_ref().is_none_or(|o| t.rel.contains(o.as_str()))
                    && wanted.as_ref().is_none_or(|w| w.contains(&t.rel))
            })
            .collect()
    }
}

/// The tests named by <work>/findings/*/finding.json.
pub fn findings_tests(work: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Ok(dir) = fs::read_dir(work.join("findings")) {
        for entry in dir.flatten() {
            if let Ok(text) = fs::read_to_string(entry.path().join("finding.json"))
                && let Ok(v) = serde_json::from_str::<Value>(&text)
                && let Some(t) = v.get("test").and_then(Value::as_str)
            {
                out.insert(t.to_owned());
            }
        }
    }
    out
}

/// What a check reports for one test.
pub trait Record: Serialize + Send {
    /// The findings, one line each (empty when there are none).
    fn findings(&self) -> Vec<String>;
    fn test(&self) -> &str;
}

/// Write a finding's directory: the test source, extra files, and `finding.json`.
pub fn write_finding(work: &Path, test: &Test, files: &[(String, Vec<u8>)], detail: &impl Serialize) {
    let dir = work.join("findings").join(test.rel.replace('/', "__"));
    let _ = fs::remove_dir_all(&dir);
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = fs::copy(&test.path, dir.join(test.file_name()));
    for (name, bytes) in files {
        let _ = fs::write(dir.join(name), bytes);
    }
    let mut value = serde_json::to_value(detail).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut value {
        map.insert("test".into(), Value::String(test.rel.clone()));
        map.insert("flags".into(), serde_json::json!(test.flags));
        map.insert("edition".into(), serde_json::json!(test.edition));
    }
    let _ = fs::write(dir.join("finding.json"), serde_json::to_string_pretty(&value).unwrap_or_default());
}

/// Run `check` over `items` on `jobs` threads; results go to <work>/results.jsonl (appended).
/// Returns exit 3 if paused at a finding, 0 otherwise.
pub fn drive<T: Sync, R: Record>(items: &[T], sweep: &Sweep, check: impl Fn(&T) -> R + Sync) -> ExitCode {
    let _ = fs::create_dir_all(&sweep.work);
    let file = OpenOptions::new().create(true).append(true).open(sweep.work.join("results.jsonl"));
    let out: Mutex<Option<BufWriter<File>>> = Mutex::new(file.ok().map(BufWriter::new));
    let stop = AtomicBool::new(false);
    let done = AtomicUsize::new(0);
    let with_findings = AtomicUsize::new(0);
    let total = items.len();
    // Large stacks: in-process parsers (syn in the rewrites) recurse as deep as a test nests.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(sweep.jobs.max(1))
        .stack_size(256 << 20)
        .build()
        .expect("thread pool");
    pool.install(|| {
        items.par_iter().for_each(|item| {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let record = check(item);
            let findings = record.findings();
            if let Ok(mut guard) = out.lock()
                && let Some(w) = guard.as_mut()
            {
                let _ = serde_json::to_writer(&mut *w, &record);
                let _ = w.write_all(b"\n");
                let _ = w.flush();
            }
            if !findings.is_empty() {
                with_findings.fetch_add(1, Ordering::Relaxed);
                println!("FINDING {}: {:?}", record.test(), findings);
                if sweep.pause_on_finding {
                    stop.store(true, Ordering::Relaxed);
                }
            }
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 100 == 0 {
                println!("{n}/{total} done, {} with findings", with_findings.load(Ordering::Relaxed));
            }
        })
    });
    let n = done.load(Ordering::Relaxed);
    let f = with_findings.load(Ordering::Relaxed);
    println!("{n} tests, {f} with findings");
    if f > 0 && sweep.pause_on_finding { ExitCode::from(3) } else { ExitCode::SUCCESS }
}

/// A per-test scratch directory, removed when dropped.
pub fn scratch_dir(sweep: &Sweep) -> tempfile::TempDir {
    tempfile::Builder::new().prefix("t").tempdir_in(sweep.scratch()).expect("scratch directory")
}

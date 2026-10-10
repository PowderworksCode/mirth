//! P6 over the UI corpus: an incremental rebuild must give what a clean build of the same source
//! gives, for every standalone UI test, with the patched compiler checking its own reuse.
//!
//! Per test, in one incremental directory: a clean session (RUSTC_VERIFY_REUSE=all, so every
//! cached value is also recomputed and compared), then three rebuilds, each with
//! RUSTC_VERIFY_REUSE=1 and RUSTC_REPORT_UNTRACKED=1:
//!
//! - unchanged: nothing edited;
//! - shift: a blank line first, which moves every span by a line;
//! - unused: an unused private fn appended.
//!
//! Each rebuild is compared with a clean build of the same source in a fresh incremental
//! directory, at the same paths: the diagnostics (as a multiset; a different order is a note),
//! the output bytes (metadata for check tests, the program otherwise), and for run tests the
//! program's output when the bytes differ, with the per-session suffixes of object names removed
//! (`.<7 chars>.rcgu.o`). A byte difference between two clean builds makes the test
//! nondeterministic, and it is skipped. Stale reuse and new untracked reads the compiler
//! reports in any session are findings too.
//!
//! The reuse check recomputes green values with their providers, and a provider that emits a
//! lint emits it again, printed without trimmed paths (the check runs under
//! `with_no_trimmed_paths`). An incremental diagnostic at the location of a clean one but worded
//! differently is that re-emission, a note; an identical extra one is a duplicate, a finding.
//! Summary lines ("N warnings emitted") are left out of the comparison.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::artifacts;
use mirth_lab::compiler_checks::{self, Known};
use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::normalize;
use mirth_lab::rustc::{Compile, Compiled, Status, observe};
use mirth_lab::uitest::{self, Kind, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    #[command(flatten)]
    sweep: Sweep,
}

static KNOWN: LazyLock<Known> = LazyLock::new(Known::load);
/// Tests that set up incremental compilation or code generation themselves, or ask for output
/// that names the session.
static OWN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"incremental|-Ccodegen-units|save-temps|-Zthreads|print-|dump-|unpretty|emit").unwrap());
/// Run tests whose output depends on more than the program (threads, time, addresses).
static SUMMARY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(warning|error): (\d+ warnings? emitted|aborting due to)").unwrap());
static LOCATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\S+:\d+:\d+: \w+)").unwrap());

/// The incremental diagnostics that are not the reuse check's re-emissions (see the module
/// comment), and how many were.
fn without_reemissions(inc: &[String], clean: &[String]) -> (Vec<String>, usize) {
    let mut rest: Vec<String> = clean.to_vec();
    let mut kept = Vec::new();
    let mut extra = Vec::new();
    for l in inc {
        if let Some(i) = rest.iter().position(|c| c == l) {
            rest.remove(i);
            kept.push(l.clone());
        } else {
            extra.push(l.clone());
        }
    }
    let loc = |l: &str| LOCATION.captures(l).map(|c| c[1].to_owned());
    let mut reemitted = 0;
    for l in extra {
        if loc(&l).is_some_and(|k| clean.iter().any(|c| loc(c).as_deref() == Some(k.as_str()) && c != &l)) {
            reemitted += 1;
        } else {
            kept.push(l);
        }
    }
    kept.sort();
    (kept, reemitted)
}

static RACY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"thread::spawn|std::thread|Instant::now|SystemTime|RandomState|HashMap|HashSet|\{:p\}|as \*const").unwrap());

#[cfg(test)]
mod tests {
    #[test]
    fn reemission_is_not_a_finding() {
        let clean = vec!["a.rs:20:13: warning: trivial cast: `&dyn Any`".to_owned()];
        let inc = vec![clean[0].clone(), "a.rs:20:13: warning: trivial cast: `&dyn std::any::Any`".to_owned()];
        assert_eq!(super::without_reemissions(&inc, &clean), (clean.clone(), 1));
        let dup = vec![clean[0].clone(), clean[0].clone()];
        assert_eq!(super::without_reemissions(&dup, &clean).0.len(), 2);
    }
}

#[derive(Serialize, Default)]
struct Rec {
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    status: String,
    found: Vec<String>,
    notes: Vec<String>,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        self.found.clone()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

const STEPS: [&str; 3] = ["unchanged", "shift", "unused"];

fn edit(step: &str, text: &str) -> Option<String> {
    match step {
        "unchanged" => Some(text.to_owned()),
        // A shebang must stay on the first line; `#![` is an inner attribute, not a shebang.
        "shift" if text.starts_with("#!") && !text.starts_with("#![") => None,
        "shift" => Some(format!("\n{text}")),
        "unused" => Some(format!("{text}\n#[allow(dead_code, unused)]\nfn __mirth_unused_fn() {{}}\n")),
        _ => None,
    }
}

struct Build {
    status: Status,
    diags: Vec<String>,
    bytes: Option<Vec<u8>>,
    stderr: String,
}

/// One session: compile `src` with incremental state in `incr`, outputs to `out/prog`.
fn build(args: &Args, test: &Test, src: &Path, out: &Path, incr: &Path, all: bool, emit: &str) -> Build {
    let _ = std::fs::remove_file(out.join("prog"));
    let c: Compiled = Compile::new(&args.rustc, src, out, &test.flags, test.edition())
        .emit(emit)
        .timeout(120)
        .compiler_checks(incr, all)
        .run();
    let stderr = compiler_checks::strip(&c.stderr);
    let mut diags: Vec<String> = stderr.lines().filter(|l| !l.trim().is_empty() && !SUMMARY.is_match(l)).map(str::to_owned).collect();
    diags.sort();
    let bytes = std::fs::read(out.join("prog")).ok().map(|b| artifacts::without_session_suffixes(&b));
    Build { status: c.status, diags, bytes, stderr: c.stderr }
}

fn check(args: &Args, test: &Test) -> Rec {
    let mut rec = Rec { test: test.rel.clone(), ..Default::default() };
    let Ok(text) = std::fs::read_to_string(&test.path) else {
        rec.skip = Some("unreadable".into());
        return rec;
    };
    let d = driver::scratch_dir(&args.sweep);
    let srcdir = d.path().join("src");
    let out = d.path().join("out");
    let _ = std::fs::create_dir_all(&srcdir);
    let _ = std::fs::create_dir_all(&out);
    let src = srcdir.join(test.file_name());
    let runs = test.kind == Some(Kind::RunPass);
    let emit = if Kind::is_check(test.kind) { "metadata" } else { "link" };
    let incr = d.path().join("incr");
    let _ = std::fs::write(&src, &text);
    let first = build(args, test, &src, &out, &incr, true, emit);
    if matches!(first.status, Status::Ice | Status::Timeout) {
        rec.skip = Some(format!("clean session: {:?}", first.status).to_lowercase());
        return rec;
    }
    rec.status = format!("{:?}", first.status).to_lowercase();
    let mut found: Vec<String> = Vec::new();
    let mut checks = compiler_checks::read(&first.stderr, &KNOWN);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for step in STEPS {
        let Some(edited) = edit(step, &text) else { continue };
        let _ = std::fs::write(&src, &edited);
        let inc = build(args, test, &src, &out, &incr, false, emit);
        let inc_prog = inc.bytes.clone();
        let more = compiler_checks::read(&inc.stderr, &KNOWN);
        checks.reuse.extend(more.reuse.iter().map(|r| format!("{step}: {r}")));
        checks.untracked.extend(more.untracked);
        checks.notes.extend(more.notes);
        let clean = |n: u32| build(args, test, &src, &out, &d.path().join(format!("clean-{step}-{n}")), false, emit);
        let c1 = clean(1);
        if inc.status != c1.status {
            found.push(format!("{step}: status: incremental {:?}, clean {:?}", inc.status, c1.status));
            files.push((format!("{step}.incremental.stderr"), inc.stderr.clone().into_bytes()));
            files.push((format!("{step}.clean.stderr"), c1.stderr.clone().into_bytes()));
            continue;
        }
        let (inc_diags, reemitted) = without_reemissions(&inc.diags, &c1.diags);
        if reemitted > 0 {
            rec.notes.push(format!("{step}: {reemitted} diagnostics re-emitted by the reuse check"));
        }
        if inc_diags != c1.diags {
            found.push(format!("{step}: diagnostics differ ({} incremental lines, {} clean)", inc_diags.len(), c1.diags.len()));
            files.push((format!("{step}.incremental.stderr"), compiler_checks::strip(&inc.stderr).into_bytes()));
            files.push((format!("{step}.clean.stderr"), compiler_checks::strip(&c1.stderr).into_bytes()));
        } else if compiler_checks::strip(&inc.stderr) != compiler_checks::strip(&c1.stderr) {
            rec.notes.push(format!("{step}: diagnostics in another order"));
        }
        if inc_prog != c1.bytes {
            // Two clean builds that differ make the comparison meaningless.
            let c2 = clean(2);
            if c2.bytes != c1.bytes {
                rec.skip = Some(format!("{step}: two clean builds differ"));
                continue;
            }
            let mut what = format!("{step}: {emit} bytes differ");
            if runs
                && !RACY.is_match(&text)
                && let (Some(a), Some(b)) = (&inc_prog, &c1.bytes)
            {
                let run = |bytes: &[u8], name: &str| {
                    let p = out.join(name);
                    let _ = std::fs::write(&p, bytes);
                    let _ = std::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o755));
                    let o = observe(&p, 20, &[]);
                    (o.exit, normalize::stdout(&o.stdout), normalize::stderr(&o.stderr))
                };
                let (oi, oc) = (run(a, "prog-incremental"), run(b, "prog-clean"));
                what.push_str(if oi == oc { " (same run output)" } else { "; the programs behave differently" });
            }
            found.push(what);
            if let Some(b) = &inc_prog {
                files.push((format!("{step}.incremental.out"), b.clone()));
            }
            if let Some(b) = &c1.bytes {
                files.push((format!("{step}.clean.out"), b.clone()));
            }
        }
    }
    found.extend(checks.findings());
    found.sort();
    found.dedup();
    checks.notes.sort();
    checks.notes.dedup();
    rec.notes.extend(checks.notes.iter().map(|n| if n.starts_with("known reuse") { n.clone() } else { format!("untracked site: {n}") }));
    if !found.is_empty() {
        driver::write_finding(&args.sweep.work, test, &files, &serde_json::json!({ "found": found, "notes": rec.notes }));
    }
    rec.found = found;
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let tests = uitest::tests(&args.sweep.tests, uitest::ALL, |t| uitest::flag_matches(t, &OWN));
    let tests = args.sweep.select(tests);
    println!("{} tests", tests.len());
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, t)))
}

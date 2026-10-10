//! Determinism: what rustc writes must depend only on its inputs and options.
//!
//! Builds each standalone UI test that compiles several times and compares the outputs
//! (`.rmeta`, `.rlib` normalized member by member, executables) with the first build:
//!
//! - repeat: the same build again, in the same directory
//! - path: the same build in another directory, both with `--remap-path-prefix` to one name
//! - threads: `-Zthreads=8` (tests marked `ignore-parallel-frontend` skip it); known: async fns
//!   (rust-lang/rust#162202), RPIT and impl Trait in traits (#163878)
//! - decoy: a `-L` directory holding unrelated libraries whose names start with the crate's
//!   name (#159677's shape)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use mirth_lab::artifacts;
use mirth_lab::driver::{self, Record, Sweep};
use mirth_lab::rustc::{Compile, Status};
use mirth_lab::uitest::{self, Kind, Test};
use regex::Regex;
use serde::Serialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// Comma-separated subset of repeat, path, threads, decoy.
    #[arg(long)]
    variants: Option<String>,
    #[command(flatten)]
    sweep: Sweep,
}

const VARIANTS: &[&str] = &["repeat", "path", "threads", "decoy"];
static OWN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"threads|remap-path|-o\b|--out-dir|emit|crate-name|extern|-L\b|-Cincremental").unwrap());
static PARALLEL_IGNORED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^//@\s*ignore-parallel-frontend").unwrap());

#[derive(Serialize)]
struct Rec {
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    found: Vec<String>,
}

impl Record for Rec {
    fn findings(&self) -> Vec<String> {
        self.found.clone()
    }
    fn test(&self) -> &str {
        &self.test
    }
}

/// Build the test copied into `src_dir`; its outputs as {file: digest}, or None if it fails.
fn build(args: &Args, test: &Test, src_dir: &Path, extra: &[String]) -> Option<BTreeMap<String, String>> {
    let out = src_dir.join("out");
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::create_dir_all(&out);
    let emit = if test.kind == Some(Kind::CheckPass) { "metadata" } else { "link,metadata" };
    let source = src_dir.join(test.file_name());
    let mut extra_all = vec![
        "--out-dir".to_string(),
        out.to_string_lossy().into_owned(),
        "--crate-name".into(),
        "t".into(),
        format!("--remap-path-prefix={}=/src", src_dir.display()),
    ];
    extra_all.extend_from_slice(extra);
    let c = Compile::new(&args.rustc, &source, &out, &test.flags, test.edition()).extra(extra_all).emit(emit).unnamed_output().run();
    (c.status == Status::Ok).then(|| artifacts::digest_dir(&out))
}

fn check(args: &Args, variants: &[&str], test: &Test) -> Rec {
    let mut rec = Rec { test: test.rel.clone(), skip: None, found: Vec::new() };
    let dir = driver::scratch_dir(&args.sweep);
    let (a, b) = (dir.path().join("a"), dir.path().join("elsewhere-b"));
    for d in [&a, &b] {
        let _ = std::fs::create_dir_all(d);
        let _ = std::fs::copy(&test.path, d.join(test.file_name()));
    }
    let Some(base) = build(args, test, &a, &[]) else {
        rec.skip = Some("does not build".into());
        return rec;
    };
    let mut found: Vec<(String, String)> = Vec::new();
    for &v in variants {
        if v == "threads" && PARALLEL_IGNORED.is_match(&test.text) {
            continue;
        }
        let got = match v {
            "repeat" => build(args, test, &a, &[]),
            "path" => build(args, test, &b, &[]),
            "threads" => build(args, test, &a, &["-Zthreads=8".into()]),
            "decoy" => {
                let decoy = dir.path().join("decoy");
                let _ = std::fs::create_dir_all(&decoy);
                let _ = std::fs::write(decoy.join("libtother.rlib"), b"!<arch>\n");
                let _ = std::fs::write(decoy.join("libt-0123456789abcdef.rmeta"), b"rust\0\0\0\0");
                build(args, test, &a, &["-L".into(), decoy.to_string_lossy().into_owned()])
            }
            _ => continue,
        };
        match got {
            None => found.push((v.into(), "does not build".into())),
            Some(g) if g != base => {
                let differ: Vec<&str> = g.keys().chain(base.keys()).filter(|k| g.get(*k) != base.get(*k)).map(String::as_str).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                found.push((v.into(), format!("outputs differ: {}", differ.join(", "))));
            }
            _ => {}
        }
    }
    // A difference also in `repeat` is the build's own nondeterminism: report only that.
    if found.iter().any(|(v, _)| v == "repeat") {
        found.retain(|(v, _)| v == "repeat");
    }
    rec.found = found.iter().map(|(v, w)| format!("{v}: {w}")).collect();
    if !found.is_empty() {
        let detail: Vec<_> = found.iter().map(|(v, w)| serde_json::json!({ "variant": v, "what": w })).collect();
        driver::write_finding(&args.sweep.work, test, &[], &serde_json::json!({ "found": detail }));
    }
    rec
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let variants: Vec<&str> = match &args.variants {
        Some(v) => v.split(',').collect(),
        None => VARIANTS.to_vec(),
    };
    let kinds = [Some(Kind::BuildPass), Some(Kind::RunPass), Some(Kind::CheckPass)];
    let tests = args.sweep.select(uitest::tests(&args.sweep.tests, &kinds, |t| uitest::flag_matches(t, &OWN)));
    println!("{} tests, variants: {}", tests.len(), variants.join(", "));
    Ok(driver::drive(&tests, &args.sweep, |t| check(&args, &variants, t)))
}

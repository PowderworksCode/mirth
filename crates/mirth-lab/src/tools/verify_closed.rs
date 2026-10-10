//! Each closed-bug query of ur/rustc must still find the code its bug's fix changed.
//!
//! For each case, fetches the files the fixing PR changed, as they were at the PR's base commit
//! (`gh`, GH_HOST=github.com), runs the query over them with Ur, and looks for a site whose
//! label and file match. Exit 1 if any case misses.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;

use anyhow::Context;
use regex::Regex;
use serde::Deserialize;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// The ur binary.
    ur: PathBuf,
    #[arg(default_value = "verify-closed")]
    work: PathBuf,
}

/// (bug, fixing PR, module, query, label contains, file contains, profile)
const CASES: &[(&str, u64, &str, &str, &str, &str, Option<&str>)] = &[
    ("#82920", 83074, "ClosedBugs", "sortByDefId", "dedup_by_key", "astconv", None),
    ("#89598", 89619, "ClosedBugs", "contextCache", "vtables_cache", "context.rs", None),
    ("#84252", 84260, "ClosedBugs", "untrackedCrateStore", "has_global_allocator", "cstore_impl", None),
    ("#40364", 71858, "ClosedBugs", "envRead", "env::var", "env.rs", Some("rust-2018")),
    ("#45841", 45899, "ClosedBugs", "writeInPlace", "out_filename", "link.rs", Some("rust-2015")),
    ("#117254", 117301, "ClosedBugs", "encoderNotFinished", "", "encoder.rs", None),
    ("#119456", 119510, "ClosedBugs", "writeErrorNotFatal", "encode_metadata", "encoder.rs", None),
    ("#34902", 35984, "ClosedBugs", "hashIterated", "xrefs", "encoder.rs", Some("rust-2015")),
    ("#65036", 65043, "RoundTrip", "hashOrderAlias", "Resolutions", "lib.rs", Some("rust-2018")),
    ("#66955", 84233, "ClosedBugs", "optionRead", "remap_path_prefix", "", None),
    ("#111227", 111641, "ClosedBugs", "fileRead", "", "debugger_visualizer", None),
];

/// Files the fix did not change but the bug's site is in.
fn extra(pr: u64) -> &'static [&'static str] {
    match pr {
        84260 => &["compiler/rustc_metadata/src/rmeta/decoder/cstore_impl.rs"],
        _ => &[],
    }
}

fn gh(args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("gh").args(args).env("GH_HOST", "github.com").output()?;
    anyhow::ensure!(out.status.success(), "gh {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrInfo {
    base_ref_oid: String,
    files: Vec<PrFile>,
}

#[derive(Deserialize)]
struct PrFile {
    path: String,
}

// Ur's grammars lack the old unstable `crate` visibility; it is not what the queries test.
static CRATE_VIS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(\s*)crate (fn|struct|enum|type|mod|use|trait|const|static|unsafe fn) ").unwrap());

fn fetch(work: &Path, pr: u64) -> anyhow::Result<PathBuf> {
    let d = work.join(pr.to_string());
    if d.exists() {
        return Ok(d);
    }
    let info: PrInfo = serde_json::from_str(&gh(&["pr", "view", &pr.to_string(), "-R", "rust-lang/rust", "--json", "baseRefOid,files"])?)?;
    let paths = info
        .files
        .iter()
        .map(|f| f.path.as_str())
        .filter(|p| p.ends_with(".rs") && !p.starts_with("tests/") && !p.starts_with("src/test/"))
        .chain(extra(pr).iter().copied());
    for path in paths {
        let target = d.join(path);
        std::fs::create_dir_all(target.parent().unwrap())?;
        let url = format!("repos/rust-lang/rust/contents/{path}?ref={}", info.base_ref_oid);
        // Added by the fix: it did not exist before.
        let Ok(text) = gh(&["api", &url, "-H", "Accept: application/vnd.github.raw"]) else { continue };
        std::fs::write(&target, CRATE_VIS.replace_all(&text, "${1}pub(crate) ${2} ").as_bytes())?;
    }
    Ok(d)
}

#[derive(Deserialize)]
struct Report {
    labels: Vec<Label>,
}

#[derive(Deserialize)]
struct Label {
    label: String,
    classifier: String,
    sites: Vec<Site>,
}

#[derive(Deserialize)]
struct Site {
    file: String,
    span: SiteSpan,
}

#[derive(Deserialize)]
struct SiteSpan {
    line: u64,
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let work = std::fs::canonicalize(&args.work)?;
    let rules = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ur/rustc");
    let mut failed = 0;
    for &(bug, pr, module, query, label, file, profile) in CASES {
        let d = fetch(&work, pr)?;
        let report = work.join("report.json");
        let mut cmd = Command::new(&args.ur);
        cmd.args(["rewrite", "--classify", "--rules"]).arg(rules.join(format!("{module}.rsc"))).arg("--report").arg(&report).arg(&d);
        if let Some(p) = profile {
            cmd.args(["--profile", p]);
        }
        let _ = std::fs::remove_file(&report);
        let ran = cmd.output()?;
        let Ok(text) = std::fs::read_to_string(&report) else {
            anyhow::bail!("{} wrote no report: {}", args.ur.display(), String::from_utf8_lossy(&ran.stderr).trim());
        };
        let r: Report = serde_json::from_str(&text).context("ur report")?;
        let hit = r
            .labels
            .iter()
            .filter(|l| l.classifier == query)
            .flat_map(|l| l.sites.iter().map(move |s| (l, s)))
            .find(|(l, s)| l.label.contains(label) && s.file.contains(file));
        failed += hit.is_none() as u32;
        let where_ = hit.map_or(String::new(), |(l, s)| {
            let name = Path::new(&s.file).file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
            format!("{} ({name}:{})", l.label.chars().take(60).collect::<String>(), s.span.line)
        });
        println!("{bug:8} {query:20} {}  {where_}", if hit.is_some() { "finds it" } else { "MISSES IT" });
    }
    Ok(if failed > 0 { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

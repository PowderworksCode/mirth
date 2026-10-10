//! The coverage build's sites by source position, and what the measured suites reached: enough to
//! ask, for any span of the compiler's source, whether code in it ran, without a rebuild.
//!
//! A block site's span is a position inside the statement or terminator the block starts at, so a
//! span (a match arm's body, a call) "ran" when a site starting inside it was reached. Spans with no
//! site inside take the nearest site before them in the same function (`probe`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::coverage::files_with;

/// A position: (line, column), both 1-based as the site tables write them.
pub type Pos = (u32, u32);

pub struct Site {
    pub id: String,
    pub path: String,
    pub pos: Pos,
    /// Only reached on a panic path (tag `panics`), or only in a logging macro (`log`).
    pub panics: bool,
    pub log: bool,
    pub snippet: String,
}

pub struct SiteMap {
    pub sites: Vec<Site>,
    /// File (`compiler/...`) -> indices into `sites`, sorted by position.
    by_file: HashMap<String, Vec<usize>>,
}

fn parse_pos(span: &str) -> Option<(String, Pos)> {
    let mut it = span.rsplitn(3, ':');
    let col = it.next()?.parse().ok()?;
    let line = it.next()?.parse().ok()?;
    Some((it.next()?.to_owned(), (line, col)))
}

impl SiteMap {
    /// The `cover` and `block` sites of the tables in `dir`.
    pub fn load(dir: &Path) -> SiteMap {
        let mut sites = Vec::new();
        let mut by_file: HashMap<String, Vec<usize>> = HashMap::new();
        for table in files_with(dir, "sites") {
            let text = String::from_utf8_lossy(&std::fs::read(&table).unwrap_or_default()).into_owned();
            for line in text.lines() {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() < 7 || (f[1] != "cover" && f[1] != "block") {
                    continue;
                }
                let Some((file, pos)) = parse_pos(f[6]) else { continue };
                let tags: Vec<&str> = if f[1] == "block" { f[5].split(' ').skip(1).collect() } else { vec![] };
                by_file.entry(file).or_default().push(sites.len());
                sites.push(Site {
                    id: f[0].into(),
                    path: f[4].into(),
                    pos,
                    panics: tags.contains(&"panics"),
                    log: tags.contains(&"log"),
                    snippet: f.get(7).unwrap_or(&"").to_string(),
                });
            }
        }
        for v in by_file.values_mut() {
            v.sort_by_key(|&i| sites[i].pos);
        }
        SiteMap { sites, by_file }
    }

    pub fn files(&self) -> impl Iterator<Item = &String> {
        self.by_file.keys()
    }

    /// The sites starting in [start, end] of `file`; when there are none, the nearest site before
    /// `start` that is not before `floor` (the enclosing function's start), if any.
    pub fn probe(&self, file: &str, start: Pos, end: Pos, floor: Pos) -> Vec<&Site> {
        let Some(v) = self.by_file.get(file) else { return Vec::new() };
        let lo = v.partition_point(|&i| self.sites[i].pos < start);
        let hi = v.partition_point(|&i| self.sites[i].pos <= end);
        if lo < hi {
            return v[lo..hi].iter().map(|&i| &self.sites[i]).collect();
        }
        match lo.checked_sub(1).map(|j| &self.sites[v[j]]) {
            Some(s) if s.pos >= floor => vec![s],
            _ => Vec::new(),
        }
    }

    /// For a file, the share of its sites whose snippet matches the source at their position:
    /// below 0.9 the source has moved since the coverage build and spans cannot be trusted.
    pub fn alignment(&self, file: &str, source: &str) -> f64 {
        let Some(v) = self.by_file.get(file) else { return 1.0 };
        let lines: Vec<&str> = source.lines().collect();
        let (mut checked, mut ok) = (0u32, 0u32);
        for &i in v.iter().step_by((v.len() / 40).max(1)) {
            let s = &self.sites[i];
            let want: String = s.snippet.chars().filter(|c| !c.is_whitespace()).take(12).collect();
            if want.is_empty() {
                continue;
            }
            let Some(line) = lines.get(s.pos.0 as usize - 1) else { continue };
            // Columns count characters; take the rest of the line and the next few lines.
            let rest: String = line.chars().skip(s.pos.1 as usize - 1).collect::<String>()
                + &lines.iter().skip(s.pos.0 as usize).take(3).copied().collect::<String>();
            let have: String = rest.chars().filter(|c| !c.is_whitespace()).take(want.chars().count()).collect();
            checked += 1;
            ok += (have == want) as u32;
        }
        if checked == 0 { 1.0 } else { ok as f64 / checked as f64 }
    }
}

/// Every site the measured runs reached: the `union.txt` of each suite under the `suites`
/// directories, and the `V <site>` lines of the raw logs under `logs` (recursively).
pub fn reached(inputs: &Inputs) -> HashSet<String> {
    let mut out = HashSet::new();
    for suites in &inputs.suites {
        let Ok(entries) = std::fs::read_dir(suites) else { continue };
        for e in entries.flatten() {
            if let Ok(text) = std::fs::read_to_string(e.path().join("union.txt")) {
                out.extend(text.split_whitespace().map(str::to_owned));
            }
        }
    }
    for dir in &inputs.logs {
        for e in walkdir::WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            if e.path().extension().is_some_and(|x| x == "log") {
                let text = String::from_utf8_lossy(&std::fs::read(e.path()).unwrap_or_default()).into_owned();
                out.extend(crate::coverage::log_hits(&text).map(str::to_owned));
            }
        }
    }
    out
}

/// The functions `mirth-lab callgraph --json` found unreachable.
pub fn unreachable(gaps_json: &Path) -> HashSet<String> {
    let text = std::fs::read_to_string(gaps_json).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    v["unreachable"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect()
}

/// The Rust files under `root` (`<rust>/compiler`), with their key as the site tables write it.
pub fn compiler_files(rust: &Path) -> Vec<(String, PathBuf)> {
    walkdir::WalkDir::new(rust.join("compiler"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .filter_map(|e| {
            let rel = e.path().strip_prefix(rust).ok()?.to_string_lossy().into_owned();
            Some((rel, e.path().to_owned()))
        })
        .collect()
}

/// Defaults shared by the reports.
#[derive(clap::Args, Debug, Clone)]
pub struct Inputs {
    /// The rust checkout whose `compiler/` the coverage build was built from (a snapshot works).
    #[arg(long, default_value_os_t = home("mirth-work/rust"))]
    pub rust: PathBuf,
    /// The coverage build's site tables.
    #[arg(long, default_value_os_t = home("mirth-work/build-blk/mirth-sites"))]
    pub sites: PathBuf,
    /// Directories of measured suites (`<suite>/union.txt`) of the block build: by default the
    /// block corpus (`cov-blk/suites`) and the suites folded since (`cov-suites`, where the checks'
    /// block runs go). A function-only run adds no block sites, so mixing it in only adds
    /// function entries.
    #[arg(long, default_values_os_t = [home("mirth-work/cov-blk/suites"), home("mirth-work/cov-suites")])]
    pub suites: Vec<PathBuf>,
    /// Directories of raw logs of the block build (searched recursively).
    #[arg(long, default_values_os_t = [home("mirth-work/cov-blk/sink"), home("mirth-work/cov-blk/flags")])]
    pub logs: Vec<PathBuf>,
    /// `mirth-lab callgraph --json` output for the block build (its `unreachable` list).
    #[arg(long, default_value_os_t = home("mirth-work/cov-blk/gaps.json"))]
    pub gaps: PathBuf,
}

pub fn home(rel: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(rel)
}

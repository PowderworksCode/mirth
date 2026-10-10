//! The survey behind docs/checks.md: the last closed rust-lang/rust bugs and the PRs that fixed
//! them (`survey-fetch`), with rollups replaced by the PR inside that names the issue
//! (`survey-unroll`). Both read and write `issues.json` in the current directory and talk to
//! GitHub through `gh` (GH_HOST=github.com).

use std::collections::HashMap;
use std::process::{Command, ExitCode};
use std::sync::LazyLock;

use anyhow::Context;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(clap::Args, Debug)]
pub struct FetchArgs {
    /// How many bugs.
    #[arg(long, default_value_t = 1000)]
    count: usize,
    /// The last day of the window, going back 30 days at a time.
    #[arg(long, default_value = "2026-10-07")]
    end: String,
}

#[derive(clap::Args, Debug)]
pub struct UnrollArgs {}

#[derive(Serialize, Deserialize, Clone)]
struct Issue {
    issue: u64,
    title: String,
    closed: String,
    labels: Vec<String>,
    body: String,
    pr: u64,
    pr_title: String,
    pr_body: String,
    pr_files: Vec<String>,
    #[serde(flatten)]
    rest: serde_json::Map<String, Value>,
}

const SEARCH: &str = r#"query($q:String!, $after:String) { search(query:$q, type:ISSUE, first:50, after:$after) {
  issueCount pageInfo { hasNextPage endCursor }
  nodes { ... on Issue { number title closedAt body
    labels(first:15) { nodes { name } }
    timelineItems(itemTypes:[CLOSED_EVENT], last:1) { nodes { ... on ClosedEvent {
      closer { ... on PullRequest { number title body merged files(first:40) { nodes { path } } }
               ... on Commit { associatedPullRequests(first:1) { nodes { number title body merged files(first:40) { nodes { path } } } } } } } } } } } } }"#;
const PR_BODY: &str = r#"query($n:Int!) { repository(owner:"rust-lang", name:"rust") { pullRequest(number:$n) { body } } }"#;
const PR: &str = r#"query($n:Int!) { repository(owner:"rust-lang", name:"rust") { pullRequest(number:$n) { number title body merged files(first:40) { nodes { path } } } } }"#;

fn graphql(fields: &[(&str, String)]) -> anyhow::Result<Value> {
    let mut cmd = Command::new("gh");
    cmd.args(["api", "graphql"]).env("GH_HOST", "github.com");
    for (flag, v) in fields {
        cmd.args([*flag, v]);
    }
    let out = cmd.output()?;
    serde_json::from_slice(&out.stdout).context("gh api graphql")
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_owned()
}

fn cut(v: &Value, n: usize) -> String {
    s(v).chars().take(n).collect()
}

fn paths(pr: &Value) -> Vec<String> {
    pr["files"]["nodes"].as_array().into_iter().flatten().map(|f| s(&f["path"])).collect()
}

fn date(d: &str) -> anyhow::Result<i64> {
    // Days since 1970-01-01 (proleptic Gregorian), and back.
    let p: Vec<i64> = d.split('-').map(|x| x.parse()).collect::<Result<_, _>>()?;
    let (y, m, day) = (p[0] - (p[1] <= 2) as i64, p[1], p[2]);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    Ok(era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468)
}

fn ymd(days: i64) -> String {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + (m <= 2) as i64)
}

pub fn fetch(args: FetchArgs) -> anyhow::Result<ExitCode> {
    let mut out: Vec<Issue> = Vec::new();
    let mut end = date(&args.end)?;
    while out.len() < args.count && end > date("2020-12-31")? {
        let start = end - 30;
        let q = format!("repo:rust-lang/rust is:issue is:closed reason:completed label:C-bug closed:{}..{}", ymd(start), ymd(end));
        let mut after: Option<String> = None;
        loop {
            let mut fields = vec![("-f", format!("query={SEARCH}")), ("-f", format!("q={q}"))];
            if let Some(a) = &after {
                fields.push(("-f", format!("after={a}")));
            }
            let r = graphql(&fields)?;
            let search = &r["data"]["search"];
            for n in search["nodes"].as_array().into_iter().flatten() {
                let closer = n["timelineItems"]["nodes"].as_array().into_iter().flatten().find_map(|t| t.get("closer").filter(|c| !c.is_null()));
                let pr = match closer {
                    Some(c) if c.get("associatedPullRequests").is_some() => c["associatedPullRequests"]["nodes"].get(0),
                    other => other,
                };
                let Some(pr) = pr.filter(|p| p["merged"].as_bool() == Some(true)) else { continue };
                out.push(Issue {
                    issue: n["number"].as_u64().unwrap_or(0),
                    title: s(&n["title"]),
                    closed: cut(&n["closedAt"], 10),
                    labels: n["labels"]["nodes"].as_array().into_iter().flatten().map(|l| s(&l["name"])).collect(),
                    body: cut(&n["body"], 2500),
                    pr: pr["number"].as_u64().unwrap_or(0),
                    pr_title: s(&pr["title"]),
                    pr_body: cut(&pr["body"], 2000),
                    pr_files: paths(pr),
                    rest: Default::default(),
                });
            }
            eprintln!("{} {}", out.len(), search["issueCount"]);
            if search["pageInfo"]["hasNextPage"].as_bool() != Some(true) {
                break;
            }
            after = Some(s(&search["pageInfo"]["endCursor"]));
        }
        end = start - 1;
    }
    out.truncate(args.count);
    std::fs::write("issues.json", serde_json::to_string(&out)?)?;
    Ok(ExitCode::SUCCESS)
}

static ISSUE_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#(\d{5,6})").unwrap());

pub fn unroll(_: UnrollArgs) -> anyhow::Result<ExitCode> {
    let mut d: Vec<Issue> = serde_json::from_str(&std::fs::read_to_string("issues.json")?)?;
    let mut bodies: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut prs: HashMap<u64, Value> = HashMap::new();
    let pr = |q: &str, n: u64| -> anyhow::Result<Value> {
        Ok(graphql(&[("-f", format!("query={q}")), ("-F", format!("n={n}"))])?["data"]["repository"]["pullRequest"].clone())
    };
    let mut fixed = 0;
    for x in d.iter_mut().filter(|x| x.pr_title.starts_with("Rollup")) {
        if !bodies.contains_key(&x.pr) {
            let body = s(&pr(PR_BODY, x.pr)?["body"]);
            let mut refs: Vec<u64> = ISSUE_REF.captures_iter(&body).filter_map(|c| c[1].parse().ok()).collect();
            refs.sort();
            refs.dedup();
            bodies.insert(x.pr, refs);
        }
        let names = Regex::new(&format!(r"#{0}\b|issues/{0}\b", x.issue))?;
        for &n in &bodies[&x.pr] {
            if !prs.contains_key(&n) {
                prs.insert(n, pr(PR, n)?);
            }
            let p = &prs[&n];
            if !p.is_null() && names.is_match(&s(&p["body"])) {
                x.pr = p["number"].as_u64().unwrap_or(n);
                x.pr_title = s(&p["title"]);
                x.pr_body = cut(&p["body"], 2000);
                x.pr_files = paths(p);
                fixed += 1;
                break;
            }
        }
    }
    std::fs::write("issues.json", serde_json::to_string(&d)?)?;
    println!("unrolled {fixed} still rollup {}", d.iter().filter(|x| x.pr_title.starts_with("Rollup")).count());
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates_round_trip() {
        for d in ["2026-10-07", "2024-02-29", "2021-01-01", "2000-03-01"] {
            assert_eq!(super::ymd(super::date(d).unwrap()), d);
        }
        assert_eq!(super::ymd(super::date("2026-03-01").unwrap() - 1), "2026-02-28");
    }
}

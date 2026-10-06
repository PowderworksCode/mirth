//! `mirth`: record a build through an instrumented compiler, and report on
//! what each compiler process did.
//!
//! ```text
//! mirth record --rustc <instrumented rustc> --out <dir> -- cargo build …
//! mirth report --sites <dir> --out <dir> [--root <path>=<name>]… [--expect <file> [--bless]]
//! mirth check  --sites <dir> --out <dir> --target-dir <dir> [--allow <file>] [--root <path>=<name>]…
//! ```
//!
//! `record` runs the command with the instrumented compiler as `RUSTC` and
//! no wrapper, so nothing between Cargo and the compiler can skip it. Each
//! compiler process writes its log to `<out>/logs`; the command's output goes
//! to `<out>/stdout` and `<out>/stderr`.
//!
//! `report` turns the logs into one list per process. With `--expect`, it
//! compares them to a blessed file and fails on any difference; `--bless`
//! writes the file instead.

mod check;
mod diff;
mod model;
mod normalize;
mod report;

use std::path::PathBuf;
use std::process::{Command, ExitCode};

struct Arguments {
    named: Vec<(String, String)>,
    switches: Vec<String>,
    rest: Vec<String>,
}

impl Arguments {
    fn parse(arguments: impl Iterator<Item = String>) -> Arguments {
        let mut parsed = Arguments {
            named: Vec::new(),
            switches: Vec::new(),
            rest: Vec::new(),
        };
        let mut arguments = arguments.peekable();
        while let Some(argument) = arguments.next() {
            if argument == "--" {
                parsed.rest.extend(arguments.by_ref());
                break;
            }
            let Some(name) = argument.strip_prefix("--") else {
                parsed.rest.push(argument);
                continue;
            };
            match arguments.peek() {
                Some(value) if !value.starts_with("--") => {
                    let value = arguments.next().unwrap_or_default();
                    parsed.named.push((name.to_owned(), value));
                }
                _ => parsed.switches.push(name.to_owned()),
            }
        }
        parsed
    }

    fn one(&self, name: &str) -> Option<&str> {
        self.named
            .iter()
            .find(|(it, _)| it == name)
            .map(|(_, value)| value.as_str())
    }

    fn all(&self, name: &str) -> Vec<&str> {
        self.named
            .iter()
            .filter(|(it, _)| it == name)
            .map(|(_, value)| value.as_str())
            .collect()
    }

    fn required(&self, name: &str) -> Result<&str, String> {
        self.one(name)
            .ok_or_else(|| format!("--{name} is required"))
    }

    fn switch(&self, name: &str) -> bool {
        self.switches.iter().any(|it| it == name)
    }
}

fn record(arguments: &Arguments) -> Result<ExitCode, String> {
    let rustc = PathBuf::from(arguments.required("rustc")?);
    let out = PathBuf::from(arguments.required("out")?);
    let Some((program, rest)) = arguments.rest.split_first() else {
        return Err("nothing to run: give the command after --".to_owned());
    };
    let _ = std::fs::remove_dir_all(out.join("logs"));
    std::fs::create_dir_all(out.join("logs")).map_err(|error| error.to_string())?;
    let ran = Command::new(program)
        .args(rest)
        .env("RUSTC", &rustc)
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "")
        .env("MIRTH_OUT", out.join("logs"))
        .output()
        .map_err(|error| format!("running {program}: {error}"))?;
    let write = |name: &str, bytes: &[u8]| {
        std::fs::write(out.join(name), bytes).map_err(|error| error.to_string())
    };
    write("stdout", &ran.stdout)?;
    write("stderr", &ran.stderr)?;
    write("status", format!("{}\n", ran.status).as_bytes())?;
    if !ran.status.success() {
        eprintln!(
            "mirth: {program} failed ({}); see {}",
            ran.status,
            out.join("stderr").display()
        );
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

fn report(arguments: &Arguments) -> Result<ExitCode, String> {
    let sites = PathBuf::from(arguments.required("sites")?);
    let out = PathBuf::from(arguments.required("out")?);
    let record = model::Record::load(&sites, &out.join("logs"))?;
    let normalize = roots(arguments)?;
    let text = report::Report {
        record: &record,
        normalize: &normalize,
    }
    .write();

    let Some(expected) = arguments.one("expect") else {
        print!("{text}");
        return Ok(ExitCode::SUCCESS);
    };
    if arguments.switch("bless") {
        std::fs::write(expected, &text).map_err(|error| format!("{expected}: {error}"))?;
        eprintln!("mirth: blessed {expected}");
        return Ok(ExitCode::SUCCESS);
    }
    let blessed = std::fs::read_to_string(expected).unwrap_or_default();
    if blessed == text {
        return Ok(ExitCode::SUCCESS);
    }
    print!("{}", diff::unified(&blessed, &text, expected));
    eprintln!("mirth: the record differs from {expected}; rerun with --bless to accept it");
    Ok(ExitCode::FAILURE)
}

fn roots(arguments: &Arguments) -> Result<normalize::Normalize, String> {
    let mut roots = Vec::new();
    for root in arguments.all("root") {
        let (path, name) = root
            .split_once('=')
            .ok_or_else(|| format!("--root {root}: expected <path>=<name>"))?;
        roots.push((path.to_owned(), name.to_owned()));
    }
    Ok(normalize::Normalize::new(roots))
}

fn check(arguments: &Arguments) -> Result<ExitCode, String> {
    let sites = PathBuf::from(arguments.required("sites")?);
    let out = PathBuf::from(arguments.required("out")?);
    let target = PathBuf::from(arguments.required("target-dir")?);
    let record = model::Record::load(&sites, &out.join("logs"))?;
    let normalize = roots(arguments)?;
    let allow = match arguments.one("allow") {
        Some(path) => check::Allow::parse(
            &std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?,
        )?,
        None => check::Allow::parse("")?,
    };
    let mut violations = check::check(&record, &normalize, &allow);
    violations.extend(check::leftovers(&target, &normalize));
    for violation in &violations {
        println!(
            "{}  {:<40} {}",
            violation.property, violation.process, violation.detail
        );
    }
    if violations.is_empty() {
        println!("P1 P2 P4 P7 hold");
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_default();
    let arguments = Arguments::parse(arguments);
    let result = match command.as_str() {
        "record" => record(&arguments),
        "report" => report(&arguments),
        "check" => check(&arguments),
        _ => Err(format!(
            "unknown command {command:?}: expected `record`, `report` or `check`"
        )),
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("mirth: {error}");
            ExitCode::from(2)
        }
    }
}

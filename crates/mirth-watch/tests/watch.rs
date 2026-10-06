//! mirth-watch driven by Cargo on `fixtures/effects`, and the program it
//! builds run with the runtime recording.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch directory");
    path
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The runtime, built once into a directory of its own.
fn runtime() -> &'static Path {
    static RUNTIME: OnceLock<PathBuf> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        let target = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("runtime");
        let built = Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "mirth-runtime", "--target-dir"])
            .arg(&target)
            .current_dir(workspace())
            .env_remove("RUSTC_WRAPPER")
            .output()
            .expect("building the runtime");
        assert!(built.status.success(), "{}", text(&built.stderr));
        target.join("debug").join("libmirth_runtime.rlib")
    })
}

struct Built {
    program: PathBuf,
    sites: Vec<Vec<String>>,
}

/// `fixtures/effects`, built through mirth-watch with its `watch.toml`.
fn build(name: &str) -> Built {
    let fixture = workspace().join("fixtures/effects");
    let root = scratch(name);
    let target = root.join("target");
    let sites = root.join("sites");
    let built = Command::new(env!("CARGO"))
        .arg("build")
        .arg("--manifest-path")
        .arg(fixture.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .env("RUSTC_WRAPPER", env!("CARGO_BIN_EXE_mirth-watch"))
        .env("MIRTH_WATCH", fixture.join("watch.toml"))
        .env("MIRTH_RUNTIME", runtime())
        .env("MIRTH_SITES", &sites)
        .output()
        .expect("running cargo");
    assert!(built.status.success(), "{}", text(&built.stderr));

    let mut rows = Vec::new();
    for file in std::fs::read_dir(&sites).expect("a site table") {
        let contents = std::fs::read_to_string(file.expect("an entry").path()).expect("reading");
        rows.extend(
            contents
                .lines()
                .map(|line| line.split('\t').map(str::to_owned).collect()),
        );
    }
    Built {
        program: target
            .join("debug")
            .join(format!("app{}", std::env::consts::EXE_SUFFIX)),
        sites: rows,
    }
}

impl Built {
    /// The id of the site of `kind` whose target is `target`.
    fn site(&self, kind: &str, target: &str) -> String {
        self.sites
            .iter()
            .find(|row| row[1] == kind && row[5] == target)
            .unwrap_or_else(|| panic!("no {kind} site for {target} in {:#?}", self.sites))[0]
            .clone()
    }

    fn run(&self, name: &str, crash: Option<String>) -> (Output, PathBuf, Vec<Vec<String>>) {
        let root = scratch(name);
        let files = root.join("files");
        let out = root.join("out");
        std::fs::create_dir_all(&files).expect("a directory for the program");
        let mut command = Command::new(&self.program);
        command
            .arg(&files)
            .env("MIRTH_OUT", &out)
            .env("STORE_LABEL", "L");
        if let Some(crash) = crash {
            command.env("MIRTH_CRASH", crash);
        }
        let ran = command.output().expect("running the program");
        let logs: Vec<_> = std::fs::read_dir(&out).expect("a log").collect();
        assert_eq!(logs.len(), 1, "one process, one log");
        let log = std::fs::read_to_string(logs[0].as_ref().expect("an entry").path())
            .expect("reading the log");
        let lines = log
            .lines()
            .map(|line| line.split('\t').map(str::to_owned).collect())
            .collect();
        (ran, files, lines)
    }
}

fn file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|it| it.to_str())
        .unwrap_or(path)
}

#[test]
fn records_frames_calls_arguments_and_statics() {
    let built = build("record");
    let (ran, _, log) = built.run("record-run", None);
    assert!(ran.status.success(), "{}", text(&ran.stderr));
    assert_eq!(text(&ran.stdout).trim(), "3 7 x LL 11");

    let save = built.site("frame", "store::save");
    let write = built.site("call", "std::fs::write");
    let rename = built.site("call", "std::fs::rename");
    let var = built.site("call", "std::env::var");
    let to_string = built.site("call", "std::string::ToString::to_string");
    let writes: Vec<String> = built
        .sites
        .iter()
        .filter(|row| row[1] == "touch" && row[5] == "store::WRITES")
        .map(|row| row[0].clone())
        .collect();
    assert_eq!(writes.len(), 2, "WRITES is touched in save and in writes");

    assert_eq!(log[0][0], "P", "the process comes first");

    let logged: Vec<(String, String, Vec<String>)> = log
        .iter()
        .filter(|row| row[0] == "L")
        .map(|row| (row[3].clone(), row[4].clone(), row[7..].to_vec()))
        .collect();
    let named: Vec<(&str, Vec<&str>)> = logged
        .iter()
        .map(|(site, frame, arguments)| {
            assert_eq!(frame, &save, "logged inside store::save");
            let kind = if *site == write { "write" } else { "rename" };
            assert!(*site == write || *site == rename);
            (kind, arguments.iter().map(|it| file_name(it)).collect())
        })
        .collect();
    assert_eq!(
        named,
        [
            ("write", vec!["a.txt.tmp"]),
            ("rename", vec!["a.txt.tmp", "a.txt"]),
            ("write", vec!["b.txt.tmp"]),
            ("rename", vec!["b.txt.tmp", "b.txt"]),
            ("write", vec!["c.txt.tmp"]),
            ("rename", vec!["c.txt.tmp", "c.txt"]),
        ]
    );
    let times: Vec<u128> = log
        .iter()
        .filter(|row| row[0] == "L")
        .map(|row| row[1].parse().expect("a time"))
        .collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]));

    type Key = (String, String, String, String, Vec<String>);
    let counted: HashMap<Key, String> = log
        .iter()
        .filter(|row| row[0] == "C")
        .map(|row| {
            (
                (
                    row[1].clone(),
                    row[2].clone(),
                    row[3].clone(),
                    row[4].clone(),
                    row[6..].to_vec(),
                ),
                row[5].clone(),
            )
        })
        .collect();
    let count = |site: &str, frame: &str, generic: &str, arguments: &[&str]| {
        let (frame, frame_arguments) = frame.split_once(' ').unwrap_or((frame, ""));
        let key = (
            site.to_owned(),
            frame.to_owned(),
            generic.to_owned(),
            frame_arguments.to_owned(),
            arguments.iter().map(|it| (*it).to_owned()).collect(),
        );
        counted
            .get(&key)
            .unwrap_or_else(|| panic!("no count for {key:?} in {counted:#?}"))
            .clone()
    };
    assert_eq!(count(&var, "0", "", &["STORE_LABEL"]), "2");
    let lookup = built.site("frame", "store::lookup");
    assert_eq!(
        count(
            &var,
            &format!("{lookup} 3:9\u{1f}Name(\"n\")"),
            "",
            &["STORE_LABEL"]
        ),
        "1",
        "inside lookup, which is named by its key's numbers and its name's Debug",
    );
    let touches = |frame: &str| -> u64 {
        counted
            .iter()
            .filter(|((site, at, _, _, _), _)| writes.contains(site) && at == frame)
            .map(|(_, count)| count.parse::<u64>().expect("a count"))
            .sum()
    };
    assert_eq!(touches(&save), 3, "touched inside save");
    assert_eq!(touches("0"), 1, "and once outside any frame");
    let encode = built.site("frame", "store::encode");
    assert_eq!(count(&to_string, &encode, "(u32,)", &[]), "1");
    assert_eq!(count(&to_string, &encode, "(&str,)", &[]), "1");
}

#[test]
fn a_point_stops_the_process_where_asked() {
    let built = build("crash");
    let rename = built.site("call", "std::fs::rename");
    let (ran, files, log) = built.run("crash-run", Some(format!("{rename}:2")));
    assert!(
        !ran.status.success(),
        "the program should have been stopped"
    );

    let mut left: Vec<String> = std::fs::read_dir(&files)
        .expect("the program's directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    left.sort();
    assert_eq!(
        left,
        ["a.txt", "b.txt.tmp"],
        "stopped before the second rename"
    );

    let logged = log.iter().filter(|row| row[0] == "L").count();
    assert_eq!(
        logged, 4,
        "written as they happened: two writes, two renames"
    );
    assert!(
        log.iter().all(|row| row[0] != "C"),
        "counts are written at exit, and an abort is not an exit"
    );
}

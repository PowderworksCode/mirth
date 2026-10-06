//! `mirth report` on a small recorded build, written by hand: one library
//! that writes its metadata, and one binary that reads it.

use std::path::Path;
use std::process::Command;

fn report(extra: &[&str]) -> (bool, String, String) {
    let record = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/record");
    let ran = Command::new(env!("CARGO_BIN_EXE_mirth"))
        .arg("report")
        .arg("--sites")
        .arg(record.join("sites"))
        .arg("--out")
        .arg(&record)
        .args(["--root", "/w/target=target"])
        .args(extra)
        .output()
        .expect("running mirth");
    (
        ran.status.success(),
        String::from_utf8_lossy(&ran.stdout).into_owned(),
        String::from_utf8_lossy(&ran.stderr).into_owned(),
    )
}

#[test]
fn lists_each_process() {
    let (ok, text, stderr) = report(&[]);
    assert!(ok, "{stderr}");
    let expected = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/record/expected.txt"),
    )
    .expect("the expected report")
    .replace("\r\n", "\n");
    assert_eq!(text, expected);
}

#[test]
fn fails_on_a_difference_and_shows_it() {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("expected-wrong.txt");
    std::fs::write(&scratch, "== nothing ==\n").expect("writing");
    let (ok, text, _) = report(&["--expect", scratch.to_str().expect("a path")]);
    assert!(!ok);
    assert!(text.contains("-== nothing =="), "{text}");
    assert!(text.contains("+== app (bin)"), "{text}");
}

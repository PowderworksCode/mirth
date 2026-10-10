//! What two runs of a program may print differently without the program behaving differently.

use std::sync::LazyLock;

use regex::Regex;

static THREAD_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(thread '[^']*') \(\d+\)").unwrap());
static STD_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\S*/lib/rustlib/src/rust/library/|/rustc/[0-9a-f]+/library/").unwrap());
static REGISTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\S*/registry/(src/)?[^/\s]+/([^/\s]+-\d[^/\s]*)/").unwrap());
static TIMING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"finished in \d+\.\d+s").unwrap());

/// Panic messages name the thread with its OS id; toolchains print std's and dependencies' paths
/// differently (in full with rust-src, remapped, relative); backtrace hints come and go.
pub fn stderr(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| {
            !l.starts_with("note: run with `RUST_BACKTRACE")
                && !l.starts_with("note: Some details are omitted")
                && !l.starts_with("note: in Miri, you may have to set `MIRIFLAGS")
        })
        .collect();
    let t = THREAD_ID.replace_all(&lines.join("\n"), "$1").into_owned();
    let t = STD_PATH.replace_all(&t, "library/").into_owned();
    REGISTRY.replace_all(&t, "<registry>/$2/").into_owned()
}

/// The test harness prints how long tests took, and its result lines in completion order.
pub fn stdout(text: &str) -> String {
    let t = TIMING.replace_all(text, "finished in …s").into_owned();
    let is_result = |l: &str| l.starts_with("test ") && l.contains(" ... ");
    let mut results: Vec<&str> = t.split('\n').filter(|l| is_result(l)).collect();
    results.sort_unstable();
    let mut it = results.into_iter();
    t.split('\n').map(|l| if is_result(l) { it.next().unwrap_or(l) } else { l }).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn thread_ids_and_paths() {
        let s = super::stderr("thread 'main' (909942) panicked at /h/.rustup/x/lib/rustlib/src/rust/library/core/src/a.rs:1:2:");
        assert_eq!(s, "thread 'main' panicked at library/core/src/a.rs:1:2:");
    }

    #[test]
    fn harness_order() {
        let s = super::stdout("test b ... ok\ntest a ... ok\nfinished in 0.12s");
        assert_eq!(s, "test a ... ok\ntest b ... ok\nfinished in …s");
    }
}

//@ needs-target-std
//
// An incremental rebuild must encode the same metadata as a clean build of the same source.
// Metadata is reused from the incremental cache when its dep-node is green, but its source
// map records each file's length and content hash, which no query tracked. So after an edit
// that changes no query result, a comment at the end of the file, the rebuild republished the
// previous session's metadata, describing a file that no longer exists.

use run_make_support::{rfs, rustc};

fn build(incremental: &str, out_dir: &str) -> Vec<u8> {
    rustc()
        .input("lib.rs")
        .crate_name("foo")
        .crate_type("lib")
        .emit("metadata,link")
        .incremental(incremental)
        .out_dir(out_dir)
        .run();
    rfs::read(format!("{out_dir}/libfoo.rmeta"))
}

fn main() {
    rfs::write("lib.rs", "pub fn f(a: u32) -> u32 { a }\n");
    let before = build("incr", "out");
    rfs::write(
        "lib.rs",
        "pub fn f(a: u32) -> u32 { a }\n// a comment at the end\n",
    );
    let incremental = build("incr", "out");
    let clean = build("clean-incr", "clean-out");
    assert!(
        incremental != before,
        "the rebuild republished the previous session's metadata"
    );
    assert!(
        incremental == clean,
        "the incremental rebuild's metadata differs from a clean build's"
    );
}

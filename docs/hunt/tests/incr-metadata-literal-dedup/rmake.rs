//@ needs-target-std
//
// An incremental rebuild must encode the same metadata as a clean build of the same source.
// Identical string literals share one `AllocId` when MIR is built (`allocate_bytes_dedup`),
// but an allocation decoded from the incremental cache got a fresh one. So after an edit
// to `b`, `a`'s MIR came from the cache and `b`'s was built again, and the metadata
// encoded the literal twice where a clean build encodes it once.

use run_make_support::{rfs, rustc};

fn build(src: &str, incremental: &str, out_dir: &str) -> Vec<u8> {
    rfs::write("lib.rs", src);
    rustc()
        .input("lib.rs")
        .crate_name("foo")
        .crate_type("lib")
        // MIR is only encoded for crates that are also code-generated, as Cargo does.
        .emit("metadata,link")
        .incremental(incremental)
        .out_dir(out_dir)
        .run();
    rfs::read(format!("{out_dir}/libfoo.rmeta"))
}

const BEFORE: &str = r#"
#[inline] pub fn a() -> &'static str { "literal" }
#[inline] pub fn b() -> &'static str { "literal" }
"#;

const AFTER: &str = r#"
#[inline] pub fn a() -> &'static str { "literal" }
#[inline] pub fn b() -> &'static str { let s = "literal"; s }
"#;

fn main() {
    build(BEFORE, "incr", "out");
    let incremental = build(AFTER, "incr", "out");
    let clean = build(AFTER, "clean-incr", "clean-out");
    assert!(
        incremental == clean,
        "the incremental rebuild's metadata differs from a clean build's"
    );
}

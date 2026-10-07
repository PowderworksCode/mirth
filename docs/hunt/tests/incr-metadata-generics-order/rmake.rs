//@ needs-target-std
//
// An incremental rebuild must encode the same metadata as a clean build of the same source.
// `Generics::param_def_id_to_index` was an `FxHashMap`, encoded in iteration order. When
// `generics_of` came from the incremental cache, decoding rebuilt the map by inserting in
// that order, which with colliding hashes gives a different layout, so the rebuild encoded
// the entries in a different order. Whether keys collide depends on their `DefIndex`es, so
// this tries the same items after 0 to 7 unrelated ones.

use run_make_support::{rfs, rustc};

fn build(src: &str, incremental: &str, out_dir: &str) -> Vec<u8> {
    rfs::write("lib.rs", src);
    rustc()
        .input("lib.rs")
        .crate_name("foo")
        .crate_type("lib")
        .emit("metadata")
        .incremental(incremental)
        .out_dir(out_dir)
        .run();
    rfs::read(format!("{out_dir}/libfoo.rmeta"))
}

fn main() {
    for padding in 0..8 {
        let mut src = String::new();
        for i in 0..padding {
            src.push_str(&format!("pub struct Padding{i};\n"));
        }
        src.push_str("pub struct Grid<A, B, C>(A, B, C);\n");
        src.push_str("impl<A, B, C> Grid<A, B, C> { pub const AREA: usize = 1; }\n");
        let edited = format!("// an edit that changes nothing but spans\n{src}");

        let (incr, out) = (format!("incr{padding}"), format!("out{padding}"));
        build(&src, &incr, &out);
        let incremental = build(&edited, &incr, &out);
        let clean = build(
            &edited,
            &format!("clean-incr{padding}"),
            &format!("clean-out{padding}"),
        );
        assert!(
            incremental == clean,
            "with {padding} padding items, the incremental rebuild's metadata differs from a clean build's"
        );
    }
}

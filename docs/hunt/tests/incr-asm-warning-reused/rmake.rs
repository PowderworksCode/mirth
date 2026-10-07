//@ needs-target-std
//@ only-x86_64
//@ ignore-cross-compile
//
// An incremental rebuild must show the same warnings as a clean build of the same source. A
// warning from inline assembly is reported by LLVM while it compiles the codegen unit, and
// nothing records it with the unit's work product, so a rebuild that reused the unit showed
// nothing.

use run_make_support::{rfs, rustc};

const MAIN: &str = "mod m;\nfn main() {\n    m::f();\n}\n";
const M: &str = "pub fn f() {\n    unsafe { std::arch::asm!(\".warning \\\"from the assembler\\\"\") }\n}\n";

fn build(incremental: &str, output: &str) -> String {
    rustc()
        .input("main.rs")
        .output(output)
        .incremental(incremental)
        .codegen_units(4)
        .run()
        .stderr_utf8()
}

fn main() {
    rfs::write("main.rs", MAIN);
    rfs::write("m.rs", M);
    assert!(build("incr", "rebuilt").contains("from the assembler"));
    rfs::write("m.rs", format!("{M}// a comment at the end\n"));
    let rebuilt = build("incr", "rebuilt");
    let clean = build("clean-incr", "clean");
    assert!(clean.contains("from the assembler"), "clean build: {clean}");
    assert!(
        rebuilt.contains("from the assembler"),
        "the incremental rebuild shows no warning from the assembler; the clean build shows:\n{clean}"
    );
}

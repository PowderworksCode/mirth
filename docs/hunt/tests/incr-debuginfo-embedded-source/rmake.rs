//@ needs-target-std
//@ ignore-windows
//@ ignore-apple
//@ ignore-wasm (`object` doesn't handle wasm object files)
//@ ignore-cross-compile
//@ ignore-backends: gcc
//
// An incremental rebuild must embed the same source as a clean build of the same source.
// A codegen unit's debuginfo names each file with its checksum and, under `-Zembed-source`,
// its contents, read from the session's source map. Adding a comment at the end of the file
// changes nothing the codegen unit depends on, so the rebuild reused the unit's object code
// and with it the file as it was before the edit.

use std::path::Path;
use std::rc::Rc;

use gimli::{EndianRcSlice, Reader, RunTimeEndian};
use object::{Object, ObjectSection};
use run_make_support::{gimli, object, rfs, rustc};

const SOURCE: &str = "fn main() {\n    println!(\"{}\", 7);\n}\n";
const EDITED: &str = "fn main() {\n    println!(\"{}\", 7);\n}\n// a comment at the end\n";

fn build(src: &str, incremental: &str, output: &str) {
    rfs::write("main.rs", src);
    rustc()
        .input("main.rs")
        .output(output)
        .incremental(incremental)
        .arg("-g")
        .arg("-Zembed-source=yes")
        .arg("-Cdwarf-version=5")
        .run();
}

/// The source embedded for `main.rs`, in every unit that embeds it.
fn embedded(output: &str) -> Vec<String> {
    let data = rfs::read(Path::new(output));
    let obj = object::File::parse(data.as_slice()).unwrap();
    let endian = if obj.is_little_endian() { RunTimeEndian::Little } else { RunTimeEndian::Big };
    let dwarf = gimli::Dwarf::load(|section| -> Result<_, ()> {
        let data = obj.section_by_name(section.name()).map(|s| s.uncompressed_data().unwrap());
        Ok(EndianRcSlice::new(Rc::from(data.unwrap_or_default().as_ref()), endian))
    })
    .unwrap();
    let mut found = Vec::new();
    let mut units = dwarf.units();
    while let Some(header) = units.next().unwrap() {
        let unit = dwarf.unit(header).unwrap();
        let unit = unit.unit_ref(&dwarf);
        let Some(program) = &unit.line_program else { continue };
        for file in program.header().file_names() {
            let name = unit.attr_string(file.path_name()).unwrap();
            if name.to_string_lossy().unwrap() != "main.rs" {
                continue;
            }
            if let Some(source) = file.source() {
                let source = unit.attr_string(source).unwrap();
                found.push(source.to_string_lossy().unwrap().to_string());
            }
        }
    }
    found
}

fn main() {
    build(SOURCE, "incr", "rebuilt");
    build(EDITED, "incr", "rebuilt");
    build(EDITED, "clean-incr", "clean");

    let clean = embedded("clean");
    assert!(!clean.is_empty() && clean.iter().all(|s| s == EDITED), "clean build: {clean:?}");
    let rebuilt = embedded("rebuilt");
    assert!(
        rebuilt.iter().all(|s| s == EDITED),
        "the incremental rebuild embeds a different main.rs from a clean build: {rebuilt:?}"
    );
}

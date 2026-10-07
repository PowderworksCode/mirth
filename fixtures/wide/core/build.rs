// Generates a table into OUT_DIR, included by lib.rs.
use std::fmt::Write;

fn main() {
    let mut table = String::from("pub const SQUARES: [u32; 16] = [");
    for i in 0..16u32 {
        write!(table, "{}, ", i * i).unwrap();
    }
    table.push_str("];\n");
    let out = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out}/table.rs"), table).unwrap();
    println!("cargo::rerun-if-changed=build.rs");
}

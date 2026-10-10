//! `mirth-rewrite <rewrite> <file.rs>`: prints the rewritten file to stdout; exits 2 when the file
//! does not parse (as `syn` sees Rust) and 3 when the rewrite does not apply to it. The rewrites
//! are documented in the library.

use std::process::exit;

use mirth_rewrite::{Outcome, REWRITES, rewrite};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: mirth-rewrite <{}> <file.rs>", REWRITES.join("|"));
        exit(64);
    }
    let text = std::fs::read_to_string(&args[2]).unwrap_or_else(|error| {
        eprintln!("{}: {error}", args[2]);
        exit(64)
    });
    match rewrite(&args[1], &text) {
        Outcome::Rewritten(out) => println!("{out}"),
        Outcome::DoesNotParse => exit(2),
        Outcome::DoesNotApply => exit(3),
        Outcome::Unknown => {
            eprintln!("unknown rewrite {}", args[1]);
            exit(64)
        }
    }
}

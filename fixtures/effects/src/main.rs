use std::path::PathBuf;

fn main() {
    let directory = PathBuf::from(std::env::args().nth(1).expect("a directory"));
    for name in ["a.txt", "b.txt", "c.txt"] {
        store::save(&directory, name, name).expect("saving");
    }
    let label = format!("{}{}", store::label(), store::label());
    println!(
        "{} {} {} {label}",
        store::writes(),
        store::encode(7u32),
        store::encode("x"),
    );
}

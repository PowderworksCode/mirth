use std::path::PathBuf;

fn main() {
    let directory = PathBuf::from(std::env::args().nth(1).expect("a directory"));
    for name in ["a.txt", "b.txt", "c.txt"] {
        store::save(&directory, name, name).expect("saving");
    }
    let label = format!("{}{}", store::label(), store::label());
    let _ = store::in_closure();
    store::tidy(&directory);
    let found = store::lookup(store::Key {
            krate: 3,
            index: store::Index::new(9),
        }, store::Name("n"));
    let paired = store::pair((
        1,
        store::Key {
            krate: 4,
            index: store::Index::new(2),
        },
    ));
    println!(
        "{} {} {} {label} {found} {paired}",
        store::writes(),
        store::encode(7u32),
        store::encode("x"),
    );
}

#![warn(let_underscore_drop)]

macro_rules! wrap {
    ($x:expr) => {
        identity($x)
    };
}

fn identity<T>(x: T) -> T {
    x
}

fn main() {
    let _ = wrap!(String::new());
}

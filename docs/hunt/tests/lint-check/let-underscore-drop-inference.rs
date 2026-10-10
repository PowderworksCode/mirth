#![warn(let_underscore_drop)]

fn main() {
    let _ = Vec::<String>::new().into_iter().collect::<Vec<_>>();
    let _: Vec<String> = Default::default();
    let _ = String::from("a").chars().rev().collect::<String>();
}

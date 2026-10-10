fn to_fn<F: Fn()>(f: F) -> F { f }
fn main() { let mut x = 0; let _f = to_fn(move || { x = 42; }); }

enum Never {}
fn f() -> Never { panic!() }
fn main() { let x = f(); let _ = x; }

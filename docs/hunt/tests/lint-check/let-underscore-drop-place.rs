#![warn(let_underscore_drop)]

struct D(&'static str);
impl Drop for D {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}

fn main() {
    let x = D("x");
    let _ = x; // a place: `x` is neither moved nor dropped here
    println!("end of main");
}

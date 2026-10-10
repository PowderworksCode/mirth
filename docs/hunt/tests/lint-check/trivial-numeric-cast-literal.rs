#![warn(trivial_numeric_casts)]

fn main() {
    // warning: trivial numeric cast: `i16` as `i16`. Without the cast, `x` is an `i32`.
    let x = 5 as i16;
    println!("{}", std::mem::size_of_val(&x));
}

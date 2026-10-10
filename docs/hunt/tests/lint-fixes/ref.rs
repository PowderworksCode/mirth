struct U;
fn main() {
    let u = U;
    let ref b = u; // unused `b`: suggestion replaces `ref b` with `_b`, which moves `u`
    drop(u);
}

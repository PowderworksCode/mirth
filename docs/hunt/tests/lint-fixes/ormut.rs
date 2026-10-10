fn main() {
    let res: &Result<u8, u8> = &Ok(1);
    match res { Ok(mut x) | &Err(mut x) => { x += 1; drop::<u8>(x) } }
    let r2: &Result<u8, u8> = &Ok(1);
    match r2 { Ok(mut y) | &Err(mut y) => drop::<u8>(y) }
}

#![crate_type = "lib"]
#![feature(repr_simd)]
#[repr(simd)]
#[derive(Copy, Clone)]
pub struct Simd<T, const N: usize>([T; N]);

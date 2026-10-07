//! Const generics, const fn, const evaluation, inline const, associated consts.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix<const R: usize, const C: usize> {
    pub cells: [[i64; C]; R],
}

impl<const R: usize, const C: usize> Matrix<R, C> {
    pub const ZERO: Self = Matrix { cells: [[0; C]; R] };
    pub const SIZE: usize = R * C;

    pub const fn identity_like() -> Self {
        let mut m = Self::ZERO;
        let mut i = 0;
        while i < R && i < C {
            m.cells[i][i] = 1;
            i += 1;
        }
        m
    }

    pub fn transpose(&self) -> Matrix<C, R> {
        let mut t = Matrix::<C, R>::ZERO;
        for i in 0..R {
            for j in 0..C {
                t.cells[j][i] = self.cells[i][j];
            }
        }
        t
    }

    pub fn mul<const K: usize>(&self, other: &Matrix<C, K>) -> Matrix<R, K> {
        let mut out = Matrix::<R, K>::ZERO;
        for i in 0..R {
            for k in 0..K {
                out.cells[i][k] = (0..C).map(|j| self.cells[i][j] * other.cells[j][k]).sum();
            }
        }
        out
    }
}

pub const fn factorial(n: u64) -> u64 {
    if n == 0 { 1 } else { n * factorial(n - 1) }
}

pub const FACT_10: u64 = factorial(10);
pub static TABLE: [u64; 6] = {
    let mut t = [0; 6];
    let mut i = 0;
    while i < 6 {
        t[i] = factorial(i as u64);
        i += 1;
    }
    t
};

pub fn first_n<const N: usize>(v: &[u8]) -> Option<[u8; N]> {
    v.get(..N)?.try_into().ok()
}

pub fn inline_const() -> [Vec<u8>; 3] {
    [const { Vec::new() }; 3]
}

pub trait HasId {
    const ID: u32;
    fn id(&self) -> u32 {
        Self::ID
    }
}

#[derive(Debug)]
pub struct A;
#[derive(Debug)]
pub struct B;
impl HasId for A {
    const ID: u32 = 1;
}
impl HasId for B {
    const ID: u32 = A::ID + 1;
}

pub const STR: &str = "a constant string";
pub const BYTES: &[u8] = b"bytes\x00\xff";
pub const C_STR: &core::ffi::CStr = c"c string";
pub const WIDE: i128 = i128::MAX / 3;
pub const FLOAT: f64 = 1.0 / 3.0;
pub const CHAR: char = '\u{1F980}';

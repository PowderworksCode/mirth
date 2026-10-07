//! Built as a dylib too, so its metadata is also embedded in a shared library.

use sink_core::consts::Matrix;

pub fn spin(m: &Matrix<2, 3>) -> Matrix<3, 2> {
    m.transpose()
}

pub fn twice(v: u32) -> u32 {
    sink_core::memory::always_inline(v) * 2
}

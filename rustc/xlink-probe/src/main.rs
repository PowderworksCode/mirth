#![no_std]
#![no_main]
#![feature(core_float_math)]
use core::fmt::Write;
use core::hint::black_box;

struct Buf { b: [u8; 256], n: usize }
impl Write for Buf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &c in s.as_bytes() { if self.n < 256 { self.b[self.n] = c; self.n += 1; } }
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn probe_entry() -> u32 {
    let a: u128 = black_box(0x1234_5678_9abc_def0_1122_3344_5566_7788);
    let b: u128 = black_box(12345);
    let i: i128 = black_box(-987654321987654321i128);
    let f: f64 = black_box(1.5e10);
    let g: f32 = black_box(2.5);
    let mut buf = Buf { b: [0; 256], n: 0 };
    let _ = write!(buf, "{} {} {} {:.3} {:e} {}", a / b, a % b, i / 7, f, g as f64, (f as i128) as f32);
    let big = black_box([7u8; 4096]);
    let mut dst = [0u8; 4096];
    dst.copy_from_slice(&big);
    let m = core::f64::math::mul_add(f, black_box(1.25), black_box(3.0)) + core::f64::math::sqrt(f) + core::f64::math::floor(f) + core::f32::math::mul_add(g, g, g) as f64;
    let x = (m as u64) + black_box(f) as u64 + black_box(g) as u64 + (i as f64) as u64;
    #[cfg(target_has_atomic = "32")]
    {
        use core::sync::atomic::{AtomicU32, Ordering};
        static C: AtomicU32 = AtomicU32::new(0);
        C.fetch_add(1, Ordering::SeqCst);
    }
    (buf.n as u32) ^ (dst[100] as u32) ^ (x as u32)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }

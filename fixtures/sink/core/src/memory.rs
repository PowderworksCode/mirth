//! Unsafe code, raw pointers, unions, repr, Drop, statics, thread locals, FFI.

use core::cell::Cell;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::sync::atomic::{AtomicU64, Ordering};

#[repr(C)]
#[derive(Clone, Copy)]
pub union IntOrFloat {
    pub i: u64,
    pub f: f64,
}

impl core::fmt::Debug for IntOrFloat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "IntOrFloat({:#x})", unsafe { self.i })
    }
}

pub fn float_bits(x: f64) -> u64 {
    unsafe { IntOrFloat { f: x }.i }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Low = 1,
    Mid = 5,
    High = 9,
}

#[non_exhaustive]
#[derive(Debug)]
pub struct Config {
    pub level: Level,
    pub name: &'static str,
}

impl Config {
    pub fn new() -> Self {
        Config { level: Level::Mid, name: "default" }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

pub static DROPS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct Loud(pub u32);

impl Drop for Loud {
    fn drop(&mut self) {
        DROPS.fetch_add(u64::from(self.0), Ordering::SeqCst);
    }
}

thread_local! {
    pub static CALLS: Cell<u32> = const { Cell::new(0) };
}

pub fn count_call() -> u32 {
    CALLS.with(|c| {
        c.set(c.get() + 1);
        c.get()
    })
}

/// Raw pointers and slices built by hand.
pub fn reverse_in_place(v: &mut [u32]) {
    let len = v.len();
    let p = v.as_mut_ptr();
    for i in 0..len / 2 {
        unsafe { core::ptr::swap(p.add(i), p.add(len - 1 - i)) };
    }
}

pub fn init_array() -> [u16; 4] {
    let mut a: [MaybeUninit<u16>; 4] = [MaybeUninit::uninit(); 4];
    for (i, slot) in a.iter_mut().enumerate() {
        slot.write(i as u16 * 7);
    }
    unsafe { core::mem::transmute::<[MaybeUninit<u16>; 4], [u16; 4]>(a) }
}

pub fn keep_alive(x: String) -> usize {
    let m = ManuallyDrop::new(x);
    let n = m.len();
    drop(ManuallyDrop::into_inner(m));
    n
}

/// Exported with a C ABI.
#[unsafe(no_mangle)]
pub extern "C" fn sink_core_add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

unsafe extern "C" {
    safe fn abs(x: i32) -> i32;
}

pub fn c_abs(x: i32) -> i32 {
    abs(x)
}

#[track_caller]
pub fn caller_line() -> u32 {
    core::panic::Location::caller().line()
}

#[must_use]
#[inline(always)]
pub fn always_inline(x: u32) -> u32 {
    x.rotate_left(3)
}

#[inline(never)]
#[cold]
pub fn never_inline(x: u32) -> u32 {
    x.reverse_bits()
}

#[deprecated(since = "0.0.0", note = "use `always_inline`")]
pub fn old(x: u32) -> u32 {
    always_inline(x)
}

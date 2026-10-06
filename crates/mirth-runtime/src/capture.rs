//! Arguments a hook can write down.
//!
//! `mirth-watch` emits `argument::<T>` only for a `T` it has checked is one
//! of these, peeling references first. MIR handed back through
//! `optimized_mir` is never type-checked again, so an argument of any other
//! type would fail in code generation rather than as an error.

use std::ffi::{OsStr, OsString};
use std::fmt::Write;
use std::path::{Path, PathBuf};

pub trait Capture {
    fn capture(&self, into: &mut String);
}

impl<T: Capture + ?Sized> Capture for &T {
    fn capture(&self, into: &mut String) {
        (**self).capture(into);
    }
}

impl<T: Capture + ?Sized> Capture for &mut T {
    fn capture(&self, into: &mut String) {
        (**self).capture(into);
    }
}

impl Capture for str {
    fn capture(&self, into: &mut String) {
        into.push_str(self);
    }
}

impl Capture for String {
    fn capture(&self, into: &mut String) {
        into.push_str(self);
    }
}

impl Capture for OsStr {
    fn capture(&self, into: &mut String) {
        into.push_str(&self.to_string_lossy());
    }
}

impl Capture for OsString {
    fn capture(&self, into: &mut String) {
        self.as_os_str().capture(into);
    }
}

impl Capture for Path {
    fn capture(&self, into: &mut String) {
        self.as_os_str().capture(into);
    }
}

impl Capture for PathBuf {
    fn capture(&self, into: &mut String) {
        self.as_os_str().capture(into);
    }
}

macro_rules! numbers {
    ($($ty:ty)*) => {$(
        impl Capture for $ty {
            fn capture(&self, into: &mut String) {
                let _ = write!(into, "{self}");
            }
        }
    )*};
}

numbers!(u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize bool char);

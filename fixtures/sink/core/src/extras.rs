//! What the edit fuzzer cannot create on its own: statics holding references and nested
//! allocations, included files, a `#[path]` module, linkage attributes, `Any`, `cfg_attr`,
//! and a generic `#[inline]` function whose MIR dependents inline from the metadata.

use core::any::Any;

#[path = "extras_inner.rs"]
pub mod inner;

pub static WORDS: &[&str] = &["alpha", "beta", "gamma"];
pub static NESTED: &[&[u8]] = &[b"ab", b"cde", &[1, 2, 3]];
pub const TABLE: &[(u8, &str)] = &[(1, "one"), (2, "two")];
pub static DATA: &str = include_str!("extras.txt");
pub static BYTES: &[u8] = include_bytes!("extras.txt");

#[unsafe(no_mangle)]
pub extern "C" fn sink_extras_add(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}

#[used]
static KEEP: [u8; 4] = *b"sink";

#[cfg_attr(feature = "extra", inline)]
pub fn lookup(n: u8) -> Option<&'static str> {
    TABLE.iter().find(|(k, _)| *k == n).map(|(_, v)| *v)
}

pub fn kind(x: &dyn Any) -> &'static str {
    if x.is::<u32>() {
        "u32"
    } else if x.is::<&str>() {
        "str"
    } else {
        "other"
    }
}

/// Generic and inlined across crates: dependents read its MIR from the metadata.
#[inline]
pub fn window<T: Copy>(v: &[T], n: usize) -> Option<&[T]> {
    v.get(..n)
}

/// Exported, and refers back to this crate through `$crate`.
#[macro_export]
macro_rules! words_len {
    () => {
        $crate::extras::WORDS.len()
    };
}

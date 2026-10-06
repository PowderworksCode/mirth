//! Stable numbers for names.

use twox_hash::XxHash3_64;

/// A number for `name` that is the same in every process and on every
/// platform: XXH3, which has a published specification, rather than `std`'s
/// hasher, which may change between releases.
pub fn identity(name: &str) -> u64 {
    XxHash3_64::oneshot(name.as_bytes())
}

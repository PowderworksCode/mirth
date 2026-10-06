//! Naming a function.
//!
//! A stable name for a body, useful without persistence — to tell whether two
//! functions are the same function, or to key a table that lives for one
//! compilation. It survives an edit to the body, and is meant to: what it
//! answers is "which function is this", not "is this still what it was".
//!
//! XXH3, from `twox-hash`. A named algorithm with a published specification
//! rather than `DefaultHasher`, because a plugin may well put these numbers
//! somewhere that outlives the compiler, and the standard library promises
//! nothing about its default hasher staying the same between releases.

use twox_hash::XxHash3_64;

/// Mirth's number for a function's identity.
pub fn identity(path: &str) -> u64 {
    XxHash3_64::oneshot(path.as_bytes())
}

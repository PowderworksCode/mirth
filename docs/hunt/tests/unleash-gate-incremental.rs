// Finding 56: a clean build rejects this crate with -Zunleash-the-miri-inside-of-you (the flag
// "may not be used to circumvent feature gates"); an incremental rebuild of the same source
// accepts it. RUSTC_BOOTSTRAP=1 rustc --crate-type lib -Zunleash-the-miri-inside-of-you
//   -Cincremental=inc --emit=metadata unleash-gate-incremental.rs   (twice; nightly-2026-10-06)
use std::sync::atomic::{AtomicUsize, Ordering};
pub const fn f(a: &AtomicUsize) -> usize { a.fetch_add(1, Ordering::Relaxed) }

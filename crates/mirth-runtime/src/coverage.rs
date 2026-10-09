//! Which instrumented functions ran: a set of sites, filled lock-free, written at exit.
//!
//! A function's first call in a process inserts its site; every later call finds it with one
//! atomic load. The table is open-addressed with linear probing and never shrinks, so a site
//! once in the table stays in its slot. Sites are 64-bit hashes; zero marks an empty slot.

use std::sync::atomic::{AtomicU64, Ordering};

const SLOTS: usize = 1 << 22;

static TABLE: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];

pub(crate) fn hit(site: u64) {
    let key = site | 1;
    let mut at = (site.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as usize & (SLOTS - 1);
    for _ in 0..SLOTS {
        let slot = &TABLE[at];
        match slot.load(Ordering::Relaxed) {
            k if k == key => return,
            0 => match slot.compare_exchange(0, key, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return,
                Err(k) if k == key => return,
                Err(_) => {}
            },
            _ => {}
        }
        at = (at + 1) & (SLOTS - 1);
    }
}

/// The sites hit so far, as they were inserted (with the low bit set).
pub(crate) fn hits() -> impl Iterator<Item = u64> {
    TABLE.iter().map(|slot| slot.load(Ordering::Relaxed)).filter(|&k| k != 0)
}

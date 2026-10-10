//! Which instrumented functions ran: a set of sites, filled lock-free, written at exit.
//!
//! A function's first call in a process inserts its site; every later call finds it with one
//! atomic load. The table is open-addressed with linear probing and never shrinks, so a site
//! once in the table stays in its slot. Sites are 64-bit hashes; zero marks an empty slot.
//!
//! Two more tables keep what a plain site cannot say. A keyed site (a block of a function whose
//! runs are told apart by a key, such as the query a generic engine function runs for) goes into
//! the site set as `site ^ mix(key)`, and into the keyed table with its parts, written as `K`
//! lines. A call pair (the instrumented function that was running when another one was entered)
//! goes into the pair table only, written as `D` lines.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

const SLOTS: usize = 1 << 22;

static TABLE: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];

fn start(site: u64, slots: usize) -> usize {
    (site.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as usize & (slots - 1)
}

/// Insert `key` (nonzero) into `table`; true when this call inserted it.
fn insert(table: &[AtomicU64], key: u64) -> bool {
    let slots = table.len();
    let mut at = start(key, slots);
    for _ in 0..slots {
        let slot = &table[at];
        match slot.load(Ordering::Relaxed) {
            k if k == key => return false,
            0 => match slot.compare_exchange(0, key, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return true,
                Err(k) if k == key => return false,
                Err(_) => {}
            },
            _ => {}
        }
        at = (at + 1) & (slots - 1);
    }
    false
}

pub(crate) fn hit(site: u64) {
    insert(&TABLE, site | 1);
}

/// The sites hit so far, as they were inserted (with the low bit set).
pub(crate) fn hits() -> impl Iterator<Item = u64> {
    TABLE.iter().map(|slot| slot.load(Ordering::Relaxed)).filter(|&k| k != 0)
}

/// A table of combined keys with the two parts each was made of, filled like the site set.
struct Parts<const N: usize> {
    keys: [AtomicU64; N],
    a: [AtomicU64; N],
    b: [AtomicU64; N],
}

impl<const N: usize> Parts<N> {
    const fn new() -> Self {
        Parts {
            keys: [const { AtomicU64::new(0) }; N],
            a: [const { AtomicU64::new(0) }; N],
            b: [const { AtomicU64::new(0) }; N],
        }
    }

    fn insert(&self, combined: u64, a: u64, b: u64) {
        let key = combined | 1;
        let mut at = start(key, N);
        for _ in 0..N {
            let slot = &self.keys[at];
            match slot.load(Ordering::Relaxed) {
                k if k == key => return,
                0 => match slot.compare_exchange(0, key, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => {
                        self.a[at].store(a, Ordering::Relaxed);
                        self.b[at].store(b, Ordering::Relaxed);
                        return;
                    }
                    Err(k) if k == key => return,
                    Err(_) => {}
                },
                _ => {}
            }
            at = (at + 1) & (N - 1);
        }
    }

    fn entries(&self) -> impl Iterator<Item = (u64, u64, u64)> + '_ {
        (0..N).filter_map(|at| {
            let key = self.keys[at].load(Ordering::Relaxed);
            (key != 0).then(|| (key, self.a[at].load(Ordering::Relaxed), self.b[at].load(Ordering::Relaxed)))
        })
    }
}

static KEYED: Parts<{ 1 << 20 }> = Parts::new();
static PAIRS: Parts<{ 1 << 22 }> = Parts::new();

fn mix(key: u64) -> u64 {
    // splitmix64's finalizer: a key of 0 or 1 still moves every bit of the site.
    let mut z = key.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A keyed site: the combined site goes into the site set, its parts into the keyed table.
pub(crate) fn hit_keyed(site: u64, key: u64) {
    let combined = (site ^ mix(key)) | 1;
    if insert(&TABLE, combined) {
        KEYED.insert(combined, site, key);
    }
}

/// The keyed sites hit so far: (combined, site, key).
pub(crate) fn keyed() -> impl Iterator<Item = (u64, u64, u64)> {
    KEYED.entries()
}

thread_local! {
    /// The site of the instrumented function running on this thread, as far as entries and
    /// returns tell (a frame left by unwinding leaves it stale until the next return).
    static CURRENT: Cell<u64> = const { Cell::new(0) };
}

/// Entering a function: record the pair (running function, this one), make this one the running
/// function, and return the one it replaces, for the matching `leave`.
pub(crate) fn enter(site: u64) -> u64 {
    let Ok(caller) = CURRENT.try_with(|current| current.replace(site)) else { return 0 };
    PAIRS.insert(mix(caller) ^ site.rotate_left(17), caller, site);
    caller
}

pub(crate) fn leave(caller: u64) {
    let _ = CURRENT.try_with(|current| current.set(caller));
}

/// The call pairs seen so far: (caller, callee); a caller of 0 is a thread's first function.
pub(crate) fn pairs() -> impl Iterator<Item = (u64, u64)> {
    PAIRS.entries().map(|(_, caller, callee)| (caller, callee))
}

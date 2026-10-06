#![feature(pattern_types, pattern_type_macro)]

use std::fmt::Display;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static WRITES: AtomicU64 = AtomicU64::new(0);
static IGNORED: AtomicU64 = AtomicU64::new(0);

/// Write a file the safe way: to a temporary file beside it, then rename.
pub fn save(directory: &Path, name: &str, text: &str) -> io::Result<()> {
    let temporary = directory.join(format!("{name}.tmp"));
    std::fs::write(&temporary, text)?;
    WRITES.fetch_add(1, Ordering::Relaxed);
    std::fs::rename(&temporary, directory.join(name))
}

pub fn writes() -> u64 {
    IGNORED.fetch_add(1, Ordering::Relaxed);
    WRITES.load(Ordering::Relaxed)
}

pub fn label() -> String {
    std::env::var("STORE_LABEL").unwrap_or_default()
}

pub fn encode<T: Display>(value: T) -> String {
    value.to_string()
}

/// Plain data: written down as its numbers.
#[derive(Clone, Copy)]
pub struct Key {
    pub krate: u32,
    pub index: Index,
}

/// An index the way rustc's index types store one: as a pattern type.
#[derive(Clone, Copy)]
pub struct Index {
    value: std::pat::pattern_type!(u32 is 0..=0xFFFF_FF00),
}

impl Index {
    pub fn new(value: u32) -> Index {
        assert!(value <= 0xFFFF_FF00);
        // SAFETY: in range, checked above.
        Index {
            value: unsafe { std::mem::transmute::<u32, _>(value) },
        }
    }

    pub fn get(self) -> u32 {
        // SAFETY: a pattern type has its base type's layout.
        unsafe { std::mem::transmute(self.value) }
    }
}

/// Written down through its `Debug`.
#[derive(Debug)]
pub struct Name(pub &'static str);

pub fn lookup(key: Key, name: Name) -> usize {
    label().len() + key.index.get() as usize + name.0.len()
}

pub fn pair(of: (u8, Key)) -> usize {
    label().len() + of.0 as usize + of.1.krate as usize
}

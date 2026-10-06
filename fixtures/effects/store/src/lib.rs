use std::fmt::Display;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static WRITES: AtomicU64 = AtomicU64::new(0);

/// Write a file the safe way: to a temporary file beside it, then rename.
pub fn save(directory: &Path, name: &str, text: &str) -> io::Result<()> {
    let temporary = directory.join(format!("{name}.tmp"));
    std::fs::write(&temporary, text)?;
    WRITES.fetch_add(1, Ordering::Relaxed);
    std::fs::rename(&temporary, directory.join(name))
}

pub fn writes() -> u64 {
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
    pub index: u32,
}

/// Written down through its `Debug`.
#[derive(Debug)]
pub struct Name(pub &'static str);

pub fn lookup(key: Key, name: Name) -> usize {
    label().len() + key.index as usize + name.0.len()
}

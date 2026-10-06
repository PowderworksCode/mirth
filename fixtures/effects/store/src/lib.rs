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

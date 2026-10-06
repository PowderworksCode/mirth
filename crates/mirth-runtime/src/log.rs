//! The one log a process writes, and what it has counted so far.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime};

type Key = (u64, u64, &'static str, Vec<String>);

struct Log {
    file: Mutex<File>,
    start: Instant,
    start_ns: u128,
    counts: Mutex<HashMap<Key, u64>>,
    arrivals: Mutex<HashMap<u64, u64>>,
    crash: Option<(u64, u64)>,
}

static LOG: OnceLock<Option<Log>> = OnceLock::new();

pub(crate) fn enabled() -> bool {
    LOG.get_or_init(open).is_some()
}

fn the() -> Option<&'static Log> {
    LOG.get().and_then(Option::as_ref)
}

fn open() -> Option<Log> {
    let directory = PathBuf::from(std::env::var_os("MIRTH_OUT")?);
    std::fs::create_dir_all(&directory).ok()?;
    let start = Instant::now();
    let start_ns = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|it| it.as_nanos())
        .unwrap_or_default();
    let pid = std::process::id();
    let file = File::options()
        .create_new(true)
        .append(true)
        .open(directory.join(format!("{pid}-{start_ns}.log")))
        .ok()?;

    let mut header = format!("P\t{pid}\t{start_ns}");
    for argument in std::env::args_os() {
        header.push('\t');
        escape(&argument.to_string_lossy(), &mut header);
    }
    header.push('\n');
    (&file).write_all(header.as_bytes()).ok()?;

    unsafe extern "C" {
        fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
    }
    // SAFETY: `finish` neither unwinds nor calls `exit`.
    unsafe { atexit(finish) };

    Some(Log {
        file: Mutex::new(file),
        start,
        start_ns,
        counts: Mutex::new(HashMap::new()),
        arrivals: Mutex::new(HashMap::new()),
        crash: crash_at(),
    })
}

fn crash_at() -> Option<(u64, u64)> {
    let spec = std::env::var("MIRTH_CRASH").ok()?;
    let (site, nth) = spec.split_once(':').unwrap_or((&spec, "1"));
    Some((site.parse().ok()?, nth.parse().ok()?))
}

fn thread() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    thread_local! {
        static THIS: u64 = NEXT.fetch_add(1, Ordering::Relaxed);
    }
    THIS.try_with(|it| *it).unwrap_or(0)
}

pub(crate) fn logged(site: u64, frame: u64, generic: &str, arguments: &[String]) {
    let Some(log) = the() else { return };
    let ns = log.start_ns + log.start.elapsed().as_nanos();
    let mut line = format!("L\t{ns}\t{}\t{site}\t{frame}\t", thread());
    escape(generic, &mut line);
    for argument in arguments {
        line.push('\t');
        escape(argument, &mut line);
    }
    line.push('\n');
    let file = log.file.lock().unwrap_or_else(|it| it.into_inner());
    let _ = (&*file).write_all(line.as_bytes());
}

pub(crate) fn counted(site: u64, frame: u64, generic: &'static str, arguments: Vec<String>) {
    let Some(log) = the() else { return };
    let mut counts = log.counts.lock().unwrap_or_else(|it| it.into_inner());
    *counts.entry((site, frame, generic, arguments)).or_insert(0) += 1;
}

pub(crate) fn arrive(site: u64) {
    let Some(log) = the() else { return };
    let Some((at, nth)) = log.crash else { return };
    if at != site {
        return;
    }
    let mut arrivals = log.arrivals.lock().unwrap_or_else(|it| it.into_inner());
    let arrived = arrivals.entry(site).or_insert(0);
    *arrived += 1;
    if *arrived == nth {
        std::process::abort();
    }
}

extern "C" fn finish() {
    let Some(log) = the() else { return };
    let counts = std::mem::take(&mut *log.counts.lock().unwrap_or_else(|it| it.into_inner()));
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort();
    let mut text = String::new();
    for ((site, frame, generic, arguments), count) in counts {
        text.push_str(&format!("C\t{site}\t{frame}\t"));
        escape(generic, &mut text);
        text.push_str(&format!("\t{count}"));
        for argument in &arguments {
            text.push('\t');
            escape(argument, &mut text);
        }
        text.push('\n');
    }
    let file = log.file.lock().unwrap_or_else(|it| it.into_inner());
    let _ = (&*file).write_all(text.as_bytes());
}

fn escape(text: &str, into: &mut String) {
    for c in text.chars() {
        match c {
            '\t' => into.push_str("\\t"),
            '\n' => into.push_str("\\n"),
            '\\' => into.push_str("\\\\"),
            c => into.push(c),
        }
    }
}

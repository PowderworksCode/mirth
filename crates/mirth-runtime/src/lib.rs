//! What instrumented code calls.
//!
//! `mirth-watch` rewrites a program's MIR to call these functions at the sites
//! its configuration names. The program never names this crate: it reaches
//! the compilation through `--extern force:`, and the plugin finds each hook
//! by its diagnostic item.
//!
//! Nothing happens unless `MIRTH_OUT` names a directory when the program
//! starts. Then each process writes one log there, `<pid>-<start>.log`:
//!
//! ```text
//! P <pid> <start ns> <argument>…                                             the process, first
//! L <ns> <thread> <site> <frame> <frame type> <frame arguments> <argument>…  a logged event, written at once
//! C <site> <frame> <frame type> <frame arguments> <count> <argument>…       a counted event, written at exit
//! ```
//!
//! Fields are separated by tabs; a tab, newline or backslash inside one is
//! escaped. A frame's arguments share one field, separated by `\x1f`.
//! Times are nanoseconds since the Unix epoch, measured as the
//! process's start time plus a monotonic offset, so they are comparable
//! across processes on one machine and never go backwards within one.
//!
//! Logged events are written as they happen, so they survive a crash.
//! Counted events are written at exit, through the C runtime's `atexit`,
//! which Linux, macOS and Windows all have: `std` runs no destructor for a
//! `static`, and `std::process::exit` runs none for anything.

#![feature(rustc_attrs)]
#![allow(internal_features)]

mod capture;
mod log;

use std::cell::{Cell, RefCell};

pub use capture::Capture;

thread_local! {
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
    static ARGUMENTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Run `work` unless this thread is already inside the runtime, or is being
/// torn down. The runtime's own calls into `std` are not instrumented, but a
/// hook can still be reached from a destructor during thread exit, when
/// thread-locals are gone.
fn guarded(work: impl FnOnce()) {
    let Ok(busy) = BUSY.try_with(|busy| busy.replace(true)) else {
        return;
    };
    if busy || !log::enabled() {
        return;
    }
    work();
    let _ = BUSY.try_with(|busy| busy.set(false));
}

#[derive(Clone)]
struct Frame {
    site: u64,
    generic: &'static str,
    arguments: String,
}

fn frame() -> Frame {
    FRAMES
        .try_with(|frames| frames.borrow().last().cloned())
        .ok()
        .flatten()
        .unwrap_or(Frame {
            site: 0,
            generic: "",
            arguments: String::new(),
        })
}

fn take_arguments() -> Vec<String> {
    ARGUMENTS
        .try_with(|arguments| std::mem::take(&mut *arguments.borrow_mut()))
        .unwrap_or_default()
}

/// Entering a frame, with the arguments captured since the last event as
/// the frame's own. `T` is the tuple of the function's type parameters, so
/// one generic function is a different frame per instantiation.
#[rustc_diagnostic_item = "mirth_enter"]
#[inline(never)]
pub fn enter<T: ?Sized>(site: u64) {
    guarded(|| {
        let generic = std::any::type_name::<T>();
        let generic = if generic == "()" { "" } else { generic };
        let arguments = take_arguments().join("\u{1f}");
        let _ = FRAMES.try_with(|frames| {
            frames.borrow_mut().push(Frame {
                site,
                generic,
                arguments,
            })
        });
    });
}

/// Leaving a frame, at a return. A frame left by unwinding is not popped
/// here; `enter` and `exit` match on the site, so a later exit of an outer
/// frame removes it too.
#[rustc_diagnostic_item = "mirth_exit"]
#[inline(never)]
pub fn exit(site: u64) {
    guarded(|| {
        let _ = FRAMES.try_with(|frames| {
            let mut frames = frames.borrow_mut();
            if let Some(at) = frames.iter().rposition(|frame| frame.site == site) {
                frames.truncate(at);
            }
        });
    });
}

/// One argument of the event about to happen at the next [`event`] call.
#[rustc_diagnostic_item = "mirth_argument"]
#[inline(never)]
pub fn argument<T: Capture + ?Sized>(value: &T) {
    guarded(|| {
        let mut text = String::new();
        value.capture(&mut text);
        let _ = ARGUMENTS.try_with(|arguments| arguments.borrow_mut().push(text));
    });
}

/// One argument, as its `Debug` renders it.
#[rustc_diagnostic_item = "mirth_argument_debug"]
#[inline(never)]
pub fn argument_debug<T: std::fmt::Debug + ?Sized>(value: &T) {
    guarded(|| {
        let text = format!("{value:?}");
        let _ = ARGUMENTS.try_with(|arguments| arguments.borrow_mut().push(text));
    });
}

/// The last `parts` arguments are the fields of one value: join them, as
/// `a:b`.
#[rustc_diagnostic_item = "mirth_join"]
#[inline(never)]
pub fn join(parts: u64) {
    guarded(|| {
        let _ = ARGUMENTS.try_with(|arguments| {
            let mut arguments = arguments.borrow_mut();
            let at = arguments.len().saturating_sub(parts as usize);
            let joined = arguments.split_off(at).join(":");
            arguments.push(joined);
        });
    });
}

/// How an event is recorded.
pub const COUNT: u64 = 0;
pub const LOG: u64 = 1;

/// An event at `site`, with the arguments captured since the last one.
#[rustc_diagnostic_item = "mirth_event"]
#[inline(never)]
pub fn event(site: u64, mode: u64) {
    guarded(|| {
        let arguments = take_arguments();
        let frame = frame();
        if mode == LOG {
            log::logged(site, &frame, &arguments);
        } else {
            log::counted(site, frame, arguments);
        }
    });
}

/// A point where a test may stop the process: `MIRTH_CRASH=<site>:<n>`
/// aborts on the `n`th arrival at `site`, counting from 1.
#[rustc_diagnostic_item = "mirth_point"]
#[inline(never)]
pub fn point(site: u64) {
    guarded(|| log::arrive(site));
}

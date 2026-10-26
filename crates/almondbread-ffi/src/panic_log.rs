//! The panic log, bounded (kb: techniques/hang-prevention "Panic log").
//!
//! Every Rust panic goes to `~/Library/Logs/Almondbread/panic.log` (the app started from Finder
//! has no stderr). A panic caught at the FFI boundary does not stop the app, so a panic that
//! fires on every display refresh used to append a full entry (message and backtrace) per
//! frame: 143 000 entries, 1.2 GB in an hour (2026-10-26, the orbit probe's
//! `DoubleExp::from_f64(inf)`). Now:
//!
//! - The first panic with a given message and location gets the full entry (backtrace) and
//!   goes to stderr through the previous hook.
//! - The same panic again only counts: one `repeated N more times` line at most every
//!   [`REPEAT_LINE_SECONDS`], and one when a different panic comes or a frame completes
//!   without one (the loop ended) or the explorer is dropped. No backtrace is captured for a
//!   repeat (it costs per frame) and nothing goes to stderr.
//! - Before each write the file is rotated at [`CAP_BYTES`]: `panic.log` -> `panic.log.1`
//!   (one old file). A file far over the cap (an old runaway log) is cut to its head first,
//!   which holds the first, informative entries.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Rotation size of `panic.log`.
pub(crate) const CAP_BYTES: u64 = 5 * 1024 * 1024;
/// A running repeat count is written at most this often.
pub(crate) const REPEAT_LINE_SECONDS: f64 = 60.0;

/// The dedupe state: the last panic's key (message + location) and its counts. Pure: the
/// caller passes the time and writes what it returns.
#[derive(Debug, Default)]
pub(crate) struct PanicDedupe {
    last: Option<String>,
    /// Times the last panic fired in all.
    total: u64,
    /// Repeats not yet reported in a line.
    pending: u64,
    /// When the last line about it was written (seconds, any clock).
    last_line: f64,
}

/// What to write for one panic.
#[derive(Debug, PartialEq)]
pub(crate) struct Decision {
    /// A repeat-count line to write first (the previous panic's, or this one's every minute).
    pub(crate) line: Option<String>,
    /// Write the full entry (message, location, backtrace): a panic unlike the last.
    pub(crate) full: bool,
}

impl PanicDedupe {
    /// A panic with `key` at `now` seconds.
    pub(crate) fn on_panic(&mut self, key: &str, now: f64) -> Decision {
        if self.last.as_deref() == Some(key) {
            self.total += 1;
            self.pending += 1;
            // (A clock step backwards counts as due, so a line still comes.)
            let elapsed = now - self.last_line;
            let due = !(0.0..REPEAT_LINE_SECONDS).contains(&elapsed);
            let line = if due { self.take_line(now) } else { None };
            return Decision { line, full: false };
        }
        let line = self.take_line(now);
        self.last = Some(key.to_string());
        self.total = 1;
        self.pending = 0;
        self.last_line = now;
        Decision { line, full: true }
    }

    /// The repeat-count line for repeats not yet reported (None if there are none).
    pub(crate) fn take_line(&mut self, now: f64) -> Option<String> {
        if self.pending == 0 {
            return None;
        }
        let key = self.last.as_deref().unwrap_or("?");
        let line = format!(
            "--- repeated {} more times ({} in all): {}",
            self.pending,
            self.total,
            key.lines().next().unwrap_or("")
        );
        self.pending = 0;
        self.last_line = now;
        Some(line)
    }

    /// Repeats wait for a line.
    pub(crate) fn has_pending(&self) -> bool {
        self.pending > 0
    }
}

/// Rotates `path` when it is `cap` bytes or larger: renamed to `<path>.1` (replacing an older
/// one). A file over twice the cap is cut to its first `cap` bytes before (the head holds the
/// first entries; the rest of a runaway log is the same entry again). Missing file: nothing.
pub(crate) fn rotate_if_full(path: &Path, cap: u64) -> std::io::Result<bool> {
    let len = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if len < cap {
        return Ok(false);
    }
    if len > cap.saturating_mul(2) {
        std::fs::OpenOptions::new().write(true).open(path)?.set_len(cap)?;
    }
    std::fs::rename(path, rotated(path))?;
    Ok(true)
}

/// `<path>.1`.
pub(crate) fn rotated(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".1");
    PathBuf::from(s)
}

/// Appends `text` (and a newline) to `path`, rotating first at `cap`.
pub(crate) fn append(path: &Path, cap: u64, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    rotate_if_full(path, cap)?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{text}")
}

static STATE: Mutex<PanicDedupe> = Mutex::new(PanicDedupe {
    last: None,
    total: 0,
    pending: 0,
    last_line: 0.0,
});
/// Mirrors "a panic was logged since the last frame that completed" for the per-frame check
/// (an atomic load, no lock, when nothing happened).
static DIRTY: AtomicBool = AtomicBool::new(false);

fn state() -> std::sync::MutexGuard<'static, PanicDedupe> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join("Library/Logs/Almondbread/panic.log"))
}

fn wall_seconds() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// The panic's text and location: what makes two panics "the same".
fn key_of(info: &std::panic::PanicHookInfo<'_>) -> String {
    let p = info.payload();
    let msg = p
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| p.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(non-string panic payload)");
    match info.location() {
        Some(l) => format!("{msg} at {}:{}:{}", l.file(), l.line(), l.column()),
        None => msg.to_string(),
    }
}

/// Installs the hook (once per process); the previous hook runs after it for new panics.
pub(crate) fn install() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let now = wall_seconds();
            let decision = state().on_panic(&key_of(info), now);
            DIRTY.store(true, Ordering::Relaxed);
            if let Some(path) = log_path() {
                if let Some(line) = &decision.line {
                    let _ = append(&path, CAP_BYTES, line);
                }
                if decision.full {
                    let bt = std::backtrace::Backtrace::force_capture();
                    let entry = format!(
                        "=== {} (unix {}) pid {} thread {:?}\n{info}\n{bt}",
                        env!("CARGO_PKG_VERSION"),
                        now as u64,
                        std::process::id(),
                        std::thread::current().name()
                    );
                    let _ = append(&path, CAP_BYTES, &entry);
                }
            }
            if decision.full {
                previous(info);
            }
        }));
    });
}

/// A frame completed without a panic: a panic loop ended (or paused). Writes its repeat count.
/// One relaxed atomic swap when nothing was logged.
pub(crate) fn frame_completed() {
    if !DIRTY.swap(false, Ordering::Relaxed) {
        return;
    }
    // The dedupe is kept: a panic that alternates with good frames (or fires in input calls
    // between them) stays one entry and a count, not one full entry each time.
    let line = state().take_line(wall_seconds());
    if let (Some(line), Some(path)) = (line, log_path()) {
        let _ = append(&path, CAP_BYTES, &format!("{line} (then a frame completed)"));
    }
}

/// Writes a pending repeat count (the explorer goes away).
pub(crate) fn flush() {
    let line = {
        let mut s = state();
        if !s.has_pending() {
            return;
        }
        s.take_line(wall_seconds())
    };
    if let (Some(line), Some(path)) = (line, log_path()) {
        let _ = append(&path, CAP_BYTES, &line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_panics_are_counted_not_logged() {
        let mut d = PanicDedupe::default();
        let a = "DoubleExp from a non-finite f64 at overlay.rs:1320:25";
        assert_eq!(d.on_panic(a, 0.0), Decision { line: None, full: true });
        // 60 frames a second for 59 s: nothing written.
        for i in 1..3540 {
            let r = d.on_panic(a, i as f64 / 60.0);
            assert_eq!(
                r,
                Decision {
                    line: None,
                    full: false
                },
                "frame {i}"
            );
        }
        // The minute is up: one line with the count.
        let r = d.on_panic(a, 60.0);
        assert!(!r.full);
        let line = r.line.expect("a line a minute");
        assert!(line.contains("repeated 3540 more times (3541 in all)"), "{line}");
        assert!(line.contains(a));
        // Then counting starts afresh.
        assert_eq!(d.on_panic(a, 61.0).line, None);
        assert_eq!(d.on_panic(a, 62.0).line, None);
        // A different panic: the previous one's count first, then its full entry.
        let r = d.on_panic("index out of bounds at host.rs:10:5", 63.0);
        assert!(r.full);
        assert!(r.line.expect("count").contains("repeated 2 more times (3543 in all)"));
        // Back to the first: full again (it is not the last any more).
        assert!(d.on_panic(a, 64.0).full);
    }

    #[test]
    fn a_completed_frame_ends_the_loop() {
        let mut d = PanicDedupe::default();
        assert!(d.on_panic("x at a.rs:1:1", 0.0).full);
        assert!(!d.on_panic("x at a.rs:1:1", 0.1).full);
        assert!(d.has_pending());
        let line = d.take_line(0.2).expect("pending count");
        assert!(line.contains("repeated 1 more times (2 in all)"), "{line}");
        assert_eq!(d.take_line(0.3), None);
        // The same panic after good frames is still a repeat (alternating panics stay small).
        assert!(!d.on_panic("x at a.rs:1:1", 1.0).full);
        // A clock step backwards does not hold the count back forever.
        d.on_panic("x at a.rs:1:1", 2.0);
        assert!(d.on_panic("x at a.rs:1:1", -100.0).line.is_some());
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let d = std::env::temp_dir().join(format!("almondbread-panic-log-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&d).expect("temp dir");
        d
    }

    #[test]
    fn the_file_rotates_at_the_cap_keeping_one_old_file() {
        let dir = temp_dir("rotate");
        let path = dir.join("logs").join("panic.log");
        let entry = "x".repeat(99);
        let cap = 1000;
        for _ in 0..55 {
            append(&path, cap, &entry).expect("append");
            let len = std::fs::metadata(&path).expect("log").len();
            assert!(len <= cap + 100, "{len}");
        }
        let old = std::fs::metadata(rotated(&path)).expect("panic.log.1").len();
        assert!((cap..cap + 100).contains(&old), "{old}");
        // No panic.log.1.1 or similar: one old file only.
        let n = std::fs::read_dir(path.parent().expect("dir")).expect("ls").count();
        assert_eq!(n, 2);
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn a_runaway_log_is_cut_to_its_head() {
        let dir = temp_dir("runaway");
        let path = dir.join("panic.log");
        let mut text = String::from("FIRST ENTRY\n");
        text.push_str(&"y".repeat(10_000));
        std::fs::write(&path, &text).expect("write");
        assert!(rotate_if_full(&path, 1000).expect("rotate"));
        let old = std::fs::read_to_string(rotated(&path)).expect("old");
        assert_eq!(old.len(), 1000);
        assert!(old.starts_with("FIRST ENTRY"));
        assert!(!path.exists());
        // Under the cap, or missing: left alone.
        assert!(!rotate_if_full(&path, 1000).expect("missing"));
        std::fs::write(&path, "small").expect("write");
        assert!(!rotate_if_full(&path, 1000).expect("small"));
        std::fs::remove_dir_all(&dir).expect("clean up");
    }
}

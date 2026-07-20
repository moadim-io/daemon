//! Bound each workbench's on-disk `agent.log` to a fixed size on the watchdog tick.
//!
//! `tmux pipe-pane -o` streams a session's raw pane output — including every ANSI redraw frame of
//! a full-screen TUI agent — into `agent.log` via an unbounded, append-only `cat >>` (see
//! `routines::command::build_routine_command`). The read side already bounds a single response to
//! `MAX_LOG_TAIL_BYTES` (#280), but nothing bounded the file's on-disk growth between TTL sweeps: a
//! long-running or chatty session could fill the disk before it was ever reaped (#268). This module
//! closes that gap by truncating an oversized log in place on the `WATCHDOG_INTERVAL` tick.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Ceiling for a single workbench's on-disk `agent.log`. Chosen well above the read-side
/// `MAX_LOG_TAIL_BYTES` (2 MiB) tail cap so a normal run's most recent output is never itself the
/// part discarded here — this only stops truly unbounded growth from a long-running or chatty
/// session between TTL sweeps (#268).
pub(crate) const MAX_AGENT_LOG_BYTES: u64 = 32 * 1024 * 1024;

/// If `path` exceeds [`MAX_AGENT_LOG_BYTES`], truncate it in place to just its last
/// `MAX_AGENT_LOG_BYTES` bytes, prefixed with a marker noting how many bytes were dropped —
/// mirroring the "recent output matters most" tradeoff the read-side tail already makes. Returns
/// whether truncation happened. A missing file or one already within budget is a no-op.
pub(crate) fn cap_agent_log_if_oversized(path: &Path) -> std::io::Result<bool> {
    cap_agent_log_to(path, MAX_AGENT_LOG_BYTES)
}

/// Shared implementation of [`cap_agent_log_if_oversized`] for an injected `max_bytes`, so tests
/// can exercise the truncation branch against a small cap instead of writing a real
/// [`MAX_AGENT_LOG_BYTES`]-sized fixture, mirroring `service_log_tail::read_log_tail_of_len`'s
/// same seam.
fn cap_agent_log_to(path: &Path, max_bytes: u64) -> std::io::Result<bool> {
    let len = match std::fs::metadata(path) {
        Ok(meta) => meta.len(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    if len <= max_bytes {
        return Ok(false);
    }
    let omitted = len - max_bytes;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    file.seek(SeekFrom::Start(omitted))?;
    let mut tail = Vec::with_capacity(max_bytes as usize);
    #[allow(
        clippy::verbose_file_reads,
        reason = "reads from the seeked offset to end-of-file for the log's tail, not the whole \
                  file `fs::read` would read"
    )]
    file.read_to_end(&mut tail)?;
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    let marker =
        format!("... [{omitted} bytes truncated; agent.log capped at {max_bytes} bytes] ...\n");
    file.write_all(marker.as_bytes())?;
    file.write_all(&tail)?;
    Ok(true)
}

/// [`cap_agent_log_if_oversized`], but best-effort: a failure (permissions, a vanishing workbench)
/// is logged and swallowed rather than propagated, matching this module's other watchdog-tick
/// helpers — a single workbench's I/O error must not abort the sweep for every other workbench.
pub(crate) fn cap_agent_log_or_warn(path: &Path) {
    if let Err(err) = cap_agent_log_if_oversized(path) {
        log::warn!("cleanup: failed to cap {}: {err}", path.display());
    }
}

#[cfg(test)]
#[path = "log_cap_tests.rs"]
mod log_cap_tests;

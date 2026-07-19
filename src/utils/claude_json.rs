//! Prune a reaped workbench's stale entry from the shared `~/.claude.json`.
//!
//! The built-in `claude` agent's `setup` step (`crate::routines::agents::claude_code`) seeds a
//! `projects[<workbench-abs-path>]` entry into `~/.claude.json` on every run, keyed by the
//! workbench's absolute path (`~/.moadim/workbenches/{slug}-{ts}`, always unique). Once the cleanup
//! sweep (`crate::routines::cleanup`) reaps that workbench directory, nothing removes the matching
//! entry, so the file grows by one dead entry per run, forever. `prune_project` removes it using the same
//! flock-guarded read -> modify -> atomic-replace pattern the setup step's python one-liner already
//! uses (`~/.claude.json.lock`, temp file + rename), so a concurrent `claude` process — or another
//! workbench's setup step running at the same time — never observes a torn or half-written file.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::paths::claude_json_path;
use crate::utils::atomic::atomic_write;

/// Remove the `projects[<workbench>]` entry (keyed by `workbench`'s absolute path) from
/// `~/.claude.json`, if both the config file and the entry exist.
///
/// Returns `Ok(true)` when an entry was found and removed, `Ok(false)` when there was nothing to
/// prune (no `~/.claude.json`, an undeterminable home directory, or no matching `projects` key) —
/// both are the common case for a workbench whose agent never ran `claude`. Returns `Err` only on an
/// actual I/O or parse failure, so the caller can log it without aborting the wider cleanup sweep.
pub fn prune_project(workbench: &Path) -> io::Result<bool> {
    prune_project_at(claude_json_path(), workbench)
}

/// Same as `prune_project`, but takes the resolved `~/.claude.json` path explicitly instead of
/// re-resolving it via `claude_json_path`, so the "home directory unresolvable" branch is
/// unit-testable without touching the real home directory (mirrors the `*_from_home` split already
/// used by `crate::paths`).
fn prune_project_at(claude_json: Option<PathBuf>, workbench: &Path) -> io::Result<bool> {
    let Some(claude_json) = claude_json else {
        return Ok(false);
    };
    if !claude_json.exists() {
        return Ok(false);
    }

    let lock_path = lock_path_for(&claude_json);
    let lock_file = File::create(&lock_path)?;
    lock_exclusive(&lock_file)?;

    let removed = prune_locked(&claude_json, workbench);

    // Best-effort: the exclusive lock is also released when `lock_file` drops at the end of this
    // function, so a failure here does not leave the file permanently locked.
    let _ = unlock(&lock_file);

    removed
}

/// Read `claude_json`, remove `workbench`'s `projects` entry if present, and atomically rewrite the
/// file when it changed. Split out of `prune_project` so the lock is held for exactly this section.
fn prune_locked(claude_json: &Path, workbench: &Path) -> io::Result<bool> {
    let raw = fs::read_to_string(claude_json)?;
    let mut document: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    let key = workbench.to_string_lossy().into_owned();
    let removed = document
        .get_mut("projects")
        .and_then(serde_json::Value::as_object_mut)
        .is_some_and(|projects| projects.remove(&key).is_some());

    if removed {
        // `document` was just parsed from valid JSON and only had a key removed, so this
        // realistically cannot fail — but propagate via `?` rather than `.expect()` since the
        // function already has an `io::Result` to carry the error through. See
        // [`SERIALIZE_FAIL_ENV`] for the test-only seam that exercises this branch.
        let bytes = serialize_document(&document)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        atomic_write(claude_json, &bytes)?;
    }

    Ok(removed)
}

/// Test-only env var: when set, [`serialize_document`] returns an error instead of serializing.
/// `serde_json::to_vec` failing on a [`serde_json::Value`] parsed from valid JSON text is otherwise
/// unreachable in a test — the only failure mode is a non-finite (NaN/Infinity) float, and JSON's
/// grammar has no way to express one — so this seam exists purely to exercise that error branch,
/// mirroring `current_exe`'s seam in `utils::process`.
#[cfg(test)]
const SERIALIZE_FAIL_ENV: &str = "MOADIM_CLAUDE_JSON_SERIALIZE_FAIL_FOR_TEST";

/// Serialize `document` back to bytes; see `SERIALIZE_FAIL_ENV` for the test-only failure seam.
fn serialize_document(document: &serde_json::Value) -> Result<Vec<u8>, serde_json::Error> {
    #[cfg(test)]
    if std::env::var_os(SERIALIZE_FAIL_ENV).is_some() {
        use serde::ser::Error as _;
        return Err(serde_json::Error::custom(
            "forced serialize failure for test",
        ));
    }
    serde_json::to_vec(document)
}

/// Path to the sibling lock file guarding `claude_json` (`~/.claude.json.lock`), matching the
/// filename the setup step's python `flock` already locks.
fn lock_path_for(claude_json: &Path) -> PathBuf {
    let mut lock = claude_json.as_os_str().to_owned();
    lock.push(".lock");
    PathBuf::from(lock)
}

/// Take a blocking exclusive advisory lock on `file`.
#[cfg(unix)]
fn lock_exclusive(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    // SAFETY: `file` owns a valid, open file descriptor for the duration of this call.
    let outcome = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if outcome == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Take a blocking exclusive advisory lock on `file`.
///
/// No-op on non-Unix targets: the daemon's launch mechanism (tmux sessions) is Unix-only, so
/// `~/.claude.json` pruning never races a concurrent writer there in practice.
#[cfg(not(unix))]
fn lock_exclusive(_file: &File) -> io::Result<()> {
    Ok(())
}

/// Release the advisory lock taken by `lock_exclusive`.
#[cfg(unix)]
fn unlock(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    // SAFETY: `file` owns a valid, open file descriptor for the duration of this call.
    let outcome = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
    if outcome == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Release the advisory lock taken by `lock_exclusive`. No-op on non-Unix targets, mirroring
/// `lock_exclusive`.
#[cfg(not(unix))]
fn unlock(_file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "claude_json_tests.rs"]
mod claude_json_tests;

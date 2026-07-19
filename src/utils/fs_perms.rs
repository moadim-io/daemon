//! Owner-only filesystem helpers.
//!
//! moadim's on-disk tree under `~/.config/moadim/` (and the run workbenches under
//! `~/.moadim/workbenches/`) is effectively a secret/transcript store: `agent.log`
//! holds the full agent transcript, `prompt.md` the operating instructions, and
//! routine state can reference tokens sourced via the login shell. Created with
//! the process's default umask those land at world-readable `0644`/`0755` — a
//! local information-disclosure vector on a shared host. This helper creates the
//! daemon's own directories owner-only (`0700`).
//!
//! Unix-only behaviour; on other platforms the call falls back to the standard
//! library with no mode tightening (the project's permission model is unix).

use std::io;
use std::path::Path;

/// Create `path` and any missing parent directories, owner-only (`0700`) on unix.
///
/// Mirrors [`std::fs::create_dir_all`] but sets mode `0700` on every directory it
/// creates. An already-existing directory is left as-is (not re-chmodded),
/// matching `create_dir_all`'s idempotent contract.
pub fn create_private_dir_all(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(path)
    }
}

/// Returns `path`'s parent directory, or an [`io::Error`] naming `what` (e.g. `"pid file"`) if
/// `path` has no parent (i.e. `path` is `/` or empty) — a condition none of this crate's
/// generated config/log/history paths hit in practice, but every writer needs an error arm for
/// it rather than a panic. Centralized here so that arm is tested once instead of once per
/// call site.
pub fn parent_or_err<'a>(path: &'a Path, what: &str) -> io::Result<&'a Path> {
    path.parent().ok_or_else(|| {
        io::Error::other(format!(
            "{what} path {} has no parent directory",
            path.display()
        ))
    })
}

#[cfg(test)]
#[path = "fs_perms_tests.rs"]
mod fs_perms_tests;

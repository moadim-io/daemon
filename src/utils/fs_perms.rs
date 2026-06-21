//! Owner-only filesystem helpers.
//!
//! moadim's on-disk tree under `~/.config/moadim/` (and the run workbenches) is
//! effectively a secret/transcript store: `agent.log` holds the full agent
//! transcript, `prompt.md` the operating instructions, and routine state can
//! reference tokens sourced via the login shell. Created with the process's
//! default umask those land at world-readable `0644`/`0755`, a local
//! information-disclosure vector on a shared host. These helpers create the
//! daemon's own dirs owner-only (`0700`) so the posture matches the `0600`
//! `mkstemp` the `~/.claude.json` setup step already uses.
//!
//! Unix-only behaviour; on other platforms the calls fall back to the standard
//! library with no mode tightening (the project's permission model is unix).

use std::io;
use std::path::Path;

/// Create `path` and any missing parent directories, owner-only (`0700`) on unix.
///
/// Mirrors [`std::fs::create_dir_all`] but sets mode `0700` on every directory
/// it creates. An already-existing directory is left as-is (not re-chmodded),
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

#[cfg(test)]
#[path = "fs_perms_tests.rs"]
mod fs_perms_tests;

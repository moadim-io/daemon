//! Atomic file writes: write to a temp file in the same directory, then rename into place.
//!
//! `std::fs::write` truncates-then-writes in place, so a crash mid-write leaves a torn file holding
//! neither the old nor the new complete contents. [`atomic_write`](crate::utils::atomic::atomic_write) avoids that by writing to a
//! uniquely-named sibling temp file and renaming it over the target, so a concurrent reader always
//! observes one complete version. This mirrors the durability guarantee the daemon already gives
//! `~/.claude.json` (write temp + `os.replace`).

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

/// Write `bytes` to `path` atomically: a reader sees either the previous contents or the complete
/// new contents, never a partial/torn file.
///
/// Writes to a uniquely-named temporary file in the **same directory** as `path` (so the final
/// rename stays on one filesystem), flushes it to disk, then renames it over `path`. The temp file
/// is removed if any step fails, so a failed write leaves no `.tmp` residue.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = tmp_path(path);
    if let Err(err) = write_tmp(&tmp, bytes) {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(())
}

/// Path to a unique sibling temp file alongside `path` (same directory, so the rename that follows
/// stays on one filesystem). The random UUID suffix avoids collisions between concurrent writers.
fn tmp_path(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("tmp");
    let mut tmp = path.to_path_buf();
    tmp.set_file_name(format!(".{name}.{}.tmp", Uuid::new_v4()));
    tmp
}

/// Create `tmp`, write all of `bytes`, and flush to disk so the rename publishes complete contents.
fn write_tmp(tmp: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = create_private(tmp)?;
    // `write_all` and `sync_all` on a freshly-created local file descriptor cannot
    // realistically fail once the `File::create` above succeeded; `.expect` avoids
    // a branch that is impossible to exercise in tests.
    file.write_all(bytes).expect("write to temp file failed");
    file.sync_all().expect("sync temp file failed");
    Ok(())
}

/// Create a fresh file at `path`, owner-only (`0600`) on unix so the published file is never
/// briefly world-readable. These files (routine state, the `prompt.md` sidecar, `machine.local.toml`)
/// can carry sensitive content and must stay owner-only on a shared host. Falls back to a plain
/// create on non-unix.
fn create_private(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
    }
    #[cfg(not(unix))]
    {
        File::create(path)
    }
}

#[cfg(test)]
#[path = "atomic_tests.rs"]
mod atomic_tests;

//! Global lock sentinel: a filesystem flag that halts all routine scheduling and triggers without
//! touching individual routine `enabled` states.
//!
//! Two variants live in `~/.config/moadim/`:
//!
//! - `.lock`       — committed; shareable across machines via version control.
//! - `.local.lock` — gitignored (the `.local.` infix matches `*.local.*`); machine-local.
//!
//! Either file's *presence* activates the lock — content is ignored. Removing both files restores
//! prior scheduling state without changing any routine's `enabled` flag.

use std::io;

/// Which lock sentinel to create or remove.
pub enum LockScope {
    /// The committed `.lock` file, shareable via version control.
    Shared,
    /// The gitignored `.local.lock` file, local to this machine.
    Local,
}

/// Current state of both lock sentinels.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct LockStatus {
    /// Whether the committed `.lock` file is present.
    pub shared: bool,
    /// Whether the gitignored `.local.lock` file is present.
    pub local: bool,
    /// `true` if either sentinel is present (all routine scheduling and triggers halted).
    pub locked: bool,
}

/// Returns `true` if either the shared (`.lock`) or local (`.local.lock`) sentinel exists.
pub fn is_globally_locked() -> bool {
    crate::paths::global_lock_path().exists() || crate::paths::global_local_lock_path().exists()
}

/// Returns the current presence state of both lock sentinels.
pub fn lock_status() -> LockStatus {
    let shared = crate::paths::global_lock_path().exists();
    let local = crate::paths::global_local_lock_path().exists();
    LockStatus {
        shared,
        local,
        locked: shared || local,
    }
}

/// Create or remove a lock sentinel.
///
/// Creating a lock writes an empty file (the daemon only checks for *presence*). Removing a lock
/// deletes the file if present; if the file is already absent this is a no-op.
pub fn set_lock(scope: LockScope, locked: bool) -> io::Result<()> {
    let path = match scope {
        LockScope::Shared => crate::paths::global_lock_path(),
        LockScope::Local => crate::paths::global_local_lock_path(),
    };
    if locked {
        std::fs::create_dir_all(crate::paths::config_dir())?;
        std::fs::write(&path, b"")?;
    } else if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "global_lock_tests.rs"]
mod tests;

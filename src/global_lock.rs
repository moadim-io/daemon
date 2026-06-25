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
#[derive(serde::Serialize)]
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
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, b"")?;
    } else if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_docs_in_private_items)]

    use super::*;

    struct TempHome(std::path::PathBuf);

    impl TempHome {
        fn set() -> Self {
            let dir =
                std::env::temp_dir().join(format!("moadim-locktest-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).expect("create temp home");
            // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
            unsafe {
                std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
            }
            TempHome(dir)
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            // SAFETY: single-threaded test execution.
            unsafe {
                std::env::remove_var("MOADIM_HOME_OVERRIDE");
            }
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn not_locked_when_no_sentinel_exists() {
        let _home = TempHome::set();
        assert!(!is_globally_locked());
        let status = lock_status();
        assert!(!status.shared);
        assert!(!status.local);
        assert!(!status.locked);
    }

    #[test]
    fn shared_lock_detected() {
        let _home = TempHome::set();
        set_lock(LockScope::Shared, true).unwrap();
        assert!(is_globally_locked());
        let status = lock_status();
        assert!(status.shared);
        assert!(!status.local);
        assert!(status.locked);
    }

    #[test]
    fn local_lock_detected() {
        let _home = TempHome::set();
        set_lock(LockScope::Local, true).unwrap();
        assert!(is_globally_locked());
        let status = lock_status();
        assert!(!status.shared);
        assert!(status.local);
        assert!(status.locked);
    }

    #[test]
    fn unlock_removes_sentinel() {
        let _home = TempHome::set();
        set_lock(LockScope::Shared, true).unwrap();
        assert!(is_globally_locked());
        set_lock(LockScope::Shared, false).unwrap();
        assert!(!is_globally_locked());
    }

    #[test]
    fn unlock_noop_when_absent() {
        let _home = TempHome::set();
        // No file present — must not error.
        set_lock(LockScope::Shared, false).unwrap();
        set_lock(LockScope::Local, false).unwrap();
        assert!(!is_globally_locked());
    }
}

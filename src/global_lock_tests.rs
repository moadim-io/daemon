#![allow(clippy::missing_docs_in_private_items)]

use super::*;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-locktest-{}", uuid::Uuid::new_v4()));
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
fn local_unlock_removes_sentinel() {
    let _home = TempHome::set();
    set_lock(LockScope::Local, true).unwrap();
    assert!(is_globally_locked());
    set_lock(LockScope::Local, false).unwrap();
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

#[cfg(unix)]
#[test]
fn set_lock_errors_when_config_dir_is_read_only() {
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();
    // Create the config dir first so `create_dir_all` (the line before L60) succeeds,
    // then strip write permission so `fs::write` for the lock sentinel (L60) fails.
    let config_dir = crate::paths::config_dir();
    std::fs::create_dir_all(&config_dir).unwrap();
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    let err = set_lock(LockScope::Shared, true).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);

    // Restore write permission so TempHome::drop can remove the temp tree.
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&config_dir, perms).unwrap();
}

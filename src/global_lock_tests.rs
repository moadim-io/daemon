#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

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
        Self(dir)
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

fn assert_recent(since: Option<u64>) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let secs = since.expect("since set for a present lock");
    assert!(
        secs.abs_diff(now) <= 60,
        "since {secs} not within 60s of {now}"
    );
}

#[test]
fn since_is_none_when_unlocked() {
    let _home = TempHome::set();
    let status = lock_status();
    assert_eq!(status.shared_since, None);
    assert_eq!(status.local_since, None);
}

#[test]
fn shared_since_set_when_shared_locked() {
    let _home = TempHome::set();
    set_lock(LockScope::Shared, true).unwrap();
    let status = lock_status();
    assert_recent(status.shared_since);
    assert_eq!(status.local_since, None);
}

#[test]
fn local_since_set_when_local_locked() {
    let _home = TempHome::set();
    set_lock(LockScope::Local, true).unwrap();
    let status = lock_status();
    assert_recent(status.local_since);
    assert_eq!(status.shared_since, None);
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

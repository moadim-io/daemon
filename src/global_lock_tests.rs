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

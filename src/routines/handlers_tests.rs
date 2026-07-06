#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use axum::extract::{Query, State};
use axum::Json;

use super::{lock, unlock, LockRequest, UnlockQuery};

/// RAII guard: sets `MOADIM_HOME_OVERRIDE` to a fresh temp dir and clears it on drop.
struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir =
            std::env::temp_dir().join(format!("moadim-handlers-test-{}", uuid::Uuid::new_v4()));
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

/// Cover line 54: `set_lock(scope, true).map_err(|_| AppError::Internal)?`
///
/// The config dir is created but made read-only so `fs::write` for the lock sentinel fails.
/// `set_lock` returns `Err`, the `map_err` converts it to `AppError::Internal`, and the `?`
/// propagates it as the handler's return value.
#[tokio::test]
async fn lock_handler_errors_when_set_lock_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();
    let config_dir = crate::paths::config_dir();
    std::fs::create_dir_all(&config_dir).unwrap();
    // Strip write permission so `fs::write` for the lock sentinel fails.
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    let store = crate::routines::new_store();
    let result = lock(
        State(store),
        Json(LockRequest {
            scope: "shared".to_string(),
        }),
    )
    .await;

    // Restore write permission so TempHome::drop can remove the temp tree.
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    assert!(
        matches!(result, Err(crate::error::AppError::Internal)),
        "expected Internal error when lock write fails"
    );
}

/// Cover line 75: `set_lock(scope, false).map_err(|_| AppError::Internal)?`
///
/// The shared lock sentinel is created, then the config dir is made read-only so
/// `fs::remove_file` fails. `set_lock` returns `Err` and the handler propagates `AppError::Internal`.
#[tokio::test]
async fn unlock_handler_errors_when_set_lock_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();
    let config_dir = crate::paths::config_dir();
    std::fs::create_dir_all(&config_dir).unwrap();
    // Create the shared sentinel so `remove_file` is attempted.
    let lock_path = crate::paths::global_lock_path();
    std::fs::write(&lock_path, b"").unwrap();
    // Strip write permission so `remove_file` fails.
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    let store = crate::routines::new_store();
    let result = unlock(
        State(store),
        Query(UnlockQuery {
            scope: "shared".to_string(),
        }),
    )
    .await;

    // Restore write permission so TempHome::drop can remove the temp tree.
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    assert!(
        matches!(result, Err(crate::error::AppError::Internal)),
        "expected Internal error when unlock fails"
    );
}

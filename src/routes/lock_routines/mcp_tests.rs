#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use rmcp::handler::server::wrapper::Parameters;

use super::MoadimMcp;
use crate::routes::mcp::mcp_types::LockRoutinesInput;

fn make_handler() -> MoadimMcp {
    MoadimMcp::new(
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        std::sync::Arc::new(tokio::sync::Notify::new()),
    )
}

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop. Tests in this crate run single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-mcptest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
    }
}

/// A crontab shim for tests: accepts `-l` (prints empty) and `-` (swallows stdin), making
/// `sync_routines_to_crontab` succeed and exercising the fall-through path after the `if let Err`.
struct SucceedingCronShim {
    base: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl SucceedingCronShim {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("moadim-scshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store = base.join("store");
        std::fs::write(&store, "").unwrap();
        let store_display = store.to_string_lossy().into_owned();
        let script = base.join("crontab-ok.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then cat \"$STORE\"; elif [ \"$1\" = \"-\" ]; then cat > \"$STORE\"; fi\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { base, previous }
    }
}

impl Drop for SucceedingCronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(val) => std::env::set_var("MOADIM_CRONTAB_BIN", val),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// A crontab shim for tests: always exits non-zero so `sync_routines_to_crontab` returns `Err`,
/// exercising the `log::warn!` path in `lock_routines`.
struct FailingCronShim {
    base: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl FailingCronShim {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("moadim-fcshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let script = base.join("crontab-fail.sh");
        std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { base, previous }
    }
}

impl Drop for FailingCronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(val) => std::env::set_var("MOADIM_CRONTAB_BIN", val),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn lock_routines_shared_creates_sentinel_and_returns_status() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["locked"], true);
    assert_eq!(val["shared"], true);
    // Clean up so other tests are not affected.
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, false).unwrap();
}

#[test]
fn lock_routines_local_creates_sentinel_and_returns_status() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "local".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["locked"], true);
    assert_eq!(val["local"], true);
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, false).unwrap();
}

#[test]
fn lock_routines_unknown_scope_is_error() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "oops".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn lock_routines_succeeds_when_crontab_sync_passes() {
    // Covers the success fall-through `}` of `if let Err(sync_err) = sync_routines_to_crontab`.
    let _home = TempHome::set();
    let _shim = SucceedingCronShim::new();
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "local".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, false).unwrap();
}

#[test]
fn lock_routines_logs_warn_when_crontab_sync_fails() {
    // Covers the `log::warn!("crontab sync after lock failed: ...")` line.
    let _home = TempHome::set();
    let _shim = FailingCronShim::new();
    let handler = make_handler();
    // The lock still succeeds even if the subsequent crontab sync fails.
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, false).unwrap();
}

#[test]
fn lock_routines_returns_error_when_set_lock_fails() {
    // Covers the `Err(error) => err(error)` IO error path in lock_routines.
    // Make set_lock fail by placing a regular file where the config dir must be created.
    let dir = std::env::temp_dir().join(format!("moadim-lockfail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    // Write a file at `.config` so create_dir_all(".config/moadim") fails.
    std::fs::write(dir.join(".config"), b"not a dir").unwrap();
    // SAFETY: single-threaded.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
    // SAFETY: cleanup.
    unsafe {
        std::env::remove_var("MOADIM_HOME_OVERRIDE");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

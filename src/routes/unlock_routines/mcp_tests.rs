#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use rmcp::handler::server::wrapper::Parameters;

use super::MoadimMcp;
use crate::routes::mcp::mcp_types::UnlockRoutinesInput;

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

#[test]
fn unlock_routines_all_removes_both_sentinels() {
    let _home = TempHome::set();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, true).unwrap();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, true).unwrap();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "all".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["locked"], false);
}

#[test]
fn unlock_routines_shared_removes_only_shared() {
    let _home = TempHome::set();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, true).unwrap();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["shared"], false);
}

#[test]
fn unlock_routines_local_removes_only_local() {
    let _home = TempHome::set();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, true).unwrap();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "local".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["local"], false);
}

#[test]
fn unlock_routines_unknown_scope_is_error() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "bad".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
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
/// exercising the `log::warn!` path in `unlock_routines`.
struct FailingCronShim {
    base: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}
include!("drop_tests.rs");

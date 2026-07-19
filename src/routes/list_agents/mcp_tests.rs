#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::MoadimMcp;

fn make_handler() -> MoadimMcp {
    MoadimMcp::new(
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        std::sync::Arc::new(tokio::sync::Notify::new()),
    )
}

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop. With no agent TOMLs present, agent listing falls back to the built-in names. Tests in
/// this crate run single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir =
            std::env::temp_dir().join(format!("moadim-listagentsmcptest-{}", uuid::Uuid::new_v4()));
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
fn list_agents_tool_returns_array() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler.list_agents().unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(val.is_array());
}

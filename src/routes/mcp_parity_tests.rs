#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn make_handler() -> MoadimMcp {
    MoadimMcp::new(
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    )
}

/// A throwaway shutdown signal for constructing a handler in tests; the `shutdown` tool fires it but
/// nothing awaits it, so notifying is a harmless no-op.
fn test_shutdown() -> crate::routes::http::ShutdownSignal {
    std::sync::Arc::new(tokio::sync::Notify::new())
}

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop. With no agent TOMLs present, agent validation falls back to the built-in names (so
/// `"claude"` is accepted) while `load_agent_command` finds no config — exercising the trigger
/// "no spawn" path without launching a real agent or writing into the user's real home. Tests in
/// this crate run single-threaded per binary, so the global env mutation is safe.
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

fn make_create_routine_req() -> crate::routines::CreateRoutineRequest {
    crate::routines::CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
        title: "Mcp Routine".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
    }
}

// ── parity tools: agents / logs / shutdown ──────────────────────────────────────

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

#[test]
fn routine_logs_tool_returns_logs_for_existing_routine() {
    use rmcp::handler::server::wrapper::Parameters;
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let handler = MoadimMcp::new(
        routines.clone(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
    let created = handler
        .create_routine(Parameters(make_create_routine_req()))
        .unwrap();
    let text = match &created.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let result = handler
        .routine_logs(Parameters(IdInput { id: id.clone() }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    // No run has executed, so there is no newest workbench log yet — empty contents.
    assert_eq!(val["logs"], "");
}

#[test]
fn routine_logs_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .routine_logs(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn get_lock_status_returns_unlocked_by_default() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler.get_lock_status().unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["locked"], false);
    assert_eq!(val["shared"], false);
    assert_eq!(val["local"], false);
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

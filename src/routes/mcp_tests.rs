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

#[test]
fn ok_helper_is_not_error() {
    let result = ok(serde_json::json!({"status": "good"}));
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn err_helper_is_error() {
    let result = err("something failed");
    assert!(result.is_error.unwrap_or(false));
}

// ── routine tools ──────────────────────────────────────────────────────────────

fn make_create_routine_req() -> crate::routines::CreateRoutineRequest {
    crate::routines::CreateRoutineRequest {
        auto_pull: true,
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

#[test]
fn list_routines_empty() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .list_routines(Parameters(ListRoutinesParam {
            local_only: None,
            include_prompts: None,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn get_routine_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .get_routine(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn create_routine_tool_invalid_cron_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let mut req = make_create_routine_req();
    req.schedule = "not-a-cron".into();
    let result = handler.create_routine(Parameters(req)).unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn create_get_update_trigger_delete_routine_success() {
    use rmcp::handler::server::wrapper::Parameters;
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let handler = MoadimMcp::new(
        routines.clone(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );

    // create
    let result = handler
        .create_routine(Parameters(make_create_routine_req()))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // get
    let result = handler
        .get_routine(Parameters(IdInput { id: id.clone() }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));

    // trigger (records the manual trigger) — while still enabled, before the disabling update below
    let result = handler
        .trigger_routine(Parameters(IdInput { id: id.clone() }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));

    // update (also disables — svc_trigger refuses a disabled routine, see
    // `trigger_routine_tool_returns_error_when_disabled`)
    let result = handler
        .update_routine(Parameters(UpdateRoutineInput {
            auto_pull: None,
            id: id.clone(),
            schedule: None,
            title: Some("Renamed".into()),
            agent: None,
            model: None,
            prompt: None,
            goal: None,
            repositories: None,
            machines: None,
            enabled: Some(false),
            ttl_secs: None,
            max_runtime_secs: None,
            tags: None,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));

    // delete
    let result = handler
        .delete_routine(Parameters(IdInput { id: id.clone() }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn cleanup_workbenches_tool_returns_removed_count() {
    let handler = make_handler();
    let result = handler.cleanup_workbenches().unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let json_str = match &result.content[0] {
        rmcp::model::ContentBlock::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(val["removed"].is_u64());
    assert!(val["freed_bytes"].is_u64());
}

#[test]
fn update_routine_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .update_routine(Parameters(UpdateRoutineInput {
            auto_pull: None,
            id: "no-such".into(),
            schedule: None,
            title: Some("x".into()),
            agent: None,
            model: None,
            prompt: None,
            goal: None,
            repositories: None,
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: None,
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn delete_routine_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .delete_routine(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn trigger_routine_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .trigger_routine(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[path = "mcp_routine_lifecycle_tests.rs"]
mod mcp_routine_lifecycle_tests;

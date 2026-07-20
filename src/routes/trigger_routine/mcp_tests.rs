#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::MoadimMcp;
use crate::routes::mcp::mcp_types::IdInput;

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

fn make_handler() -> MoadimMcp {
    MoadimMcp::new(
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        std::sync::Arc::new(tokio::sync::Notify::new()),
    )
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

#[test]
fn trigger_routine_tool_returns_error_when_disabled() {
    use rmcp::handler::server::wrapper::Parameters;
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let handler = MoadimMcp::new(
        routines,
        crate::paths::routines_dir(),
        0,
        std::sync::Arc::new(tokio::sync::Notify::new()),
    );

    let result = handler
        .create_routine(Parameters(crate::routines::CreateRoutineRequest {
            enabled: false,
            ..make_create_routine_req()
        }))
        .unwrap();
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let result = handler.trigger_routine(Parameters(IdInput { id })).unwrap();
    assert!(result.is_error.unwrap_or(false));
}

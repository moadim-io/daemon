#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use rmcp::handler::server::wrapper::Parameters;

use super::MoadimMcp;
use crate::routes::mcp::mcp_types::CreateFlagInput;

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
        env: std::collections::HashMap::new(),
    }
}

fn result_json(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    serde_json::from_str(&text).unwrap()
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
fn create_flag_not_found_is_error() {
    let handler = make_handler();
    let result = handler
        .create_flag(Parameters(CreateFlagInput {
            id: "no-such".into(),
            r#type: "bug".into(),
            description: "d".into(),
            scope: "general".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn create_flag_invalid_scope_is_error() {
    let _home = TempHome::set();
    let handler = make_handler();
    let created = handler
        .create_routine(Parameters(make_create_routine_req()))
        .unwrap();
    let id = result_json(&created)["id"].as_str().unwrap().to_string();

    let result = handler
        .create_flag(Parameters(CreateFlagInput {
            id,
            r#type: "bug".into(),
            description: "d".into(),
            scope: "nowhere".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

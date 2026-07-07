#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use rmcp::handler::server::wrapper::Parameters;

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
/// `"claude"` is accepted) while `load_agent_command` finds no config. Tests in this crate run
/// single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir =
            std::env::temp_dir().join(format!("moadim-mcp-preview-test-{}", uuid::Uuid::new_v4()));
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

/// `preview_routine_prompt`'s `Err` branch (issue #391): an unknown routine ID surfaces as an
/// error `CallToolResult`, mirroring `get_routine_not_found_is_error`.
#[test]
fn preview_routine_prompt_not_found_is_error() {
    let handler = make_handler();
    let result = handler
        .preview_routine_prompt(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

/// `preview_routine_prompt`'s `Ok` branch: the composed prompt body for a real routine, with no
/// workbench created and no agent launched.
#[test]
fn preview_routine_prompt_success_contains_prompt() {
    let _home = TempHome::set();
    let handler = make_handler();

    let create_result = handler
        .create_routine(Parameters(crate::routines::CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            title: "Preview Routine".into(),
            agent: "claude".into(),
            prompt: "do the thing".into(),
            goal: None,
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
        }))
        .unwrap();
    assert!(!create_result.is_error.unwrap_or(false));
    let text = match &create_result.content[0] {
        rmcp::model::ContentBlock::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let result = handler
        .preview_routine_prompt(Parameters(IdInput { id }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let preview: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(preview["prompt"]
        .as_str()
        .unwrap()
        .trim_end()
        .ends_with("do the thing"));
}

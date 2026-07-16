#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

//! Snooze/power-saving/run-listing MCP tool tests, split out of `mcp_tests.rs` to stay under the
//! repo's 500-line-per-file cap. A child of `mcp_tests` (not a sibling of `mcp`) so it inherits its
//! `make_handler`/`test_shutdown`/`TempHome`/`make_create_routine_req` fixtures via `use super::*;`.

use super::*;

#[test]
fn snooze_routine_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .snooze_routine(Parameters(SnoozeRoutineInput {
            id: "no-such".into(),
            snoozed_until: Some(1),
            skip_runs: None,
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn snooze_routine_tool_both_modes_set_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let handler = MoadimMcp::new(
        routines.clone(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );

    let result = handler
        .create_routine(Parameters(make_create_routine_req()))
        .unwrap();
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let result = handler
        .snooze_routine(Parameters(SnoozeRoutineInput {
            id,
            snoozed_until: Some(1),
            skip_runs: Some(1),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn snooze_routine_tool_sets_and_clears_snooze() {
    use rmcp::handler::server::wrapper::Parameters;
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let handler = MoadimMcp::new(
        routines.clone(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );

    let result = handler
        .create_routine(Parameters(make_create_routine_req()))
        .unwrap();
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // skip_runs mode
    let result = handler
        .snooze_routine(Parameters(SnoozeRoutineInput {
            id: id.clone(),
            snoozed_until: None,
            skip_runs: Some(3),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    assert_eq!(
        routines.lock().unwrap().get(&id).unwrap().skip_runs,
        Some(3)
    );

    // clear (both None)
    let result = handler
        .snooze_routine(Parameters(SnoozeRoutineInput {
            id: id.clone(),
            snoozed_until: None,
            skip_runs: None,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    assert_eq!(routines.lock().unwrap().get(&id).unwrap().skip_runs, None);
}

#[test]
fn trigger_routine_tool_returns_error_when_disabled() {
    use rmcp::handler::server::wrapper::Parameters;
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let handler = MoadimMcp::new(
        routines.clone(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
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

#[test]
fn set_power_saving_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .set_power_saving(Parameters(SetPowerSavingInput {
            id: "no-such".into(),
            active: true,
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn set_power_saving_tool_blocks_trigger_without_touching_enabled() {
    use rmcp::handler::server::wrapper::Parameters;
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let handler = MoadimMcp::new(
        routines.clone(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );

    let result = handler
        .create_routine(Parameters(make_create_routine_req()))
        .unwrap();
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let result = handler
        .set_power_saving(Parameters(SetPowerSavingInput {
            id: id.clone(),
            active: true,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    assert!(routines.lock().unwrap().get(&id).unwrap().enabled);

    let result = handler
        .trigger_routine(Parameters(IdInput { id: id.clone() }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));

    // clear
    let result = handler
        .set_power_saving(Parameters(SetPowerSavingInput {
            id: id.clone(),
            active: false,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    assert!(!routines.lock().unwrap().get(&id).unwrap().power_saving);

    let result = handler.trigger_routine(Parameters(IdInput { id })).unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn list_routine_runs_tool_returns_empty_list_for_existing_routine() {
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
        .list_routine_runs(Parameters(IdInput { id }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        rmcp::model::ContentBlock::Text(block) => block.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val, serde_json::json!([]));
}

#[test]
fn list_routine_runs_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .list_routine_runs(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

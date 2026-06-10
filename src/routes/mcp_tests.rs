#![allow(clippy::missing_docs_in_private_items)]

use crate::cron_jobs::{new_registry, new_store};

use super::*;

fn make_handler() -> MoadimMcp {
    MoadimMcp::new(new_store(), new_registry(), 0)
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

#[test]
fn health_returns_success() {
    let handler = make_handler();
    let result = handler.health().unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn health_content_contains_status() {
    let handler = make_handler();
    let result = handler.health().unwrap();
    let text = &result.content[0];
    let json_str = match &text.raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["status"], "ok");
    assert_eq!(val["running"], true);
}

#[test]
fn echo_tool_returns_message() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .echo(Parameters(EchoInput {
            message: "test-msg".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["message"], "test-msg");
}

#[test]
fn list_cron_jobs_empty() {
    let handler = make_handler();
    let result = handler.list_cron_jobs().unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(val.as_array().unwrap().is_empty());
}

#[test]
fn get_cron_job_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .get_cron_job(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn delete_cron_job_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .delete_cron_job(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn trigger_cron_job_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .trigger_cron_job(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn list_system_cron_jobs_returns_success() {
    let handler = make_handler();
    let result = handler.list_system_cron_jobs().unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

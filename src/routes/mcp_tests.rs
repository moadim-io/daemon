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
fn create_cron_job_tool_invalid_cron_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let store = crate::cron_jobs::new_store();
    let handler = MoadimMcp::new(store, new_registry(), 0);
    let req = crate::cron_jobs::CreateRequest {
        schedule: "not-a-cron".into(),
        handler: "h".into(),
        metadata: serde_json::Value::Null,
        enabled: true,
    };
    let result = handler.create_cron_job(Parameters(req)).unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn update_cron_job_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = MoadimMcp::new(crate::cron_jobs::new_store(), new_registry(), 0);
    let result = handler
        .update_cron_job(Parameters(UpdateInput {
            id: "no-such".into(),
            schedule: None,
            handler: Some("h".into()),
            metadata: None,
            enabled: None,
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn delete_cron_job_tool_success() {
    use rmcp::handler::server::wrapper::Parameters;
    let store = crate::cron_jobs::new_store();
    let created = crate::cron_jobs::svc_create(
        &store,
        &new_registry(),
        crate::cron_jobs::CreateRequest {
            schedule: "@daily".into(),
            handler: "h".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();
    let handler = MoadimMcp::new(store, new_registry(), 0);
    let result = handler.delete_cron_job(Parameters(IdInput { id })).unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn trigger_cron_job_tool_success() {
    use rmcp::handler::server::wrapper::Parameters;
    let store = crate::cron_jobs::new_store();
    let created = crate::cron_jobs::svc_create(
        &store,
        &new_registry(),
        crate::cron_jobs::CreateRequest {
            schedule: "@daily".into(),
            handler: "h".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();
    let handler = MoadimMcp::new(store, new_registry(), 0);
    let result = handler
        .trigger_cron_job(Parameters(IdInput { id: id.clone() }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    crate::storage::remove_job_dir(&id).unwrap();
}

#[test]
fn create_cron_job_tool_success() {
    use rmcp::handler::server::wrapper::Parameters;
    let store = crate::cron_jobs::new_store();
    let handler = MoadimMcp::new(store, new_registry(), 0);
    let req = crate::cron_jobs::CreateRequest {
        schedule: "@daily".into(),
        handler: "mcp-handler".into(),
        metadata: serde_json::Value::Null,
        enabled: true,
    };
    let result = handler.create_cron_job(Parameters(req)).unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    let id = val["id"].as_str().unwrap().to_string();
    crate::storage::remove_job_dir(&id).unwrap();
}

#[test]
fn get_cron_job_tool_success() {
    use rmcp::handler::server::wrapper::Parameters;
    let store = crate::cron_jobs::new_store();
    // Insert a job directly into the store (no disk I/O needed for get)
    let job = crate::cron_jobs::CronJob {
        id: "get-test-id".into(),
        schedule: "@daily".into(),
        handler: "h".into(),
        metadata: serde_json::Value::Null,
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
    };
    store.lock().unwrap().insert("get-test-id".into(), job);
    let handler = MoadimMcp::new(store, new_registry(), 0);
    let result = handler
        .get_cron_job(Parameters(IdInput {
            id: "get-test-id".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn update_cron_job_tool_success() {
    use rmcp::handler::server::wrapper::Parameters;
    let store = crate::cron_jobs::new_store();
    let handler = MoadimMcp::new(store.clone(), new_registry(), 0);
    // Create a job first
    let created = crate::cron_jobs::svc_create(
        &store,
        &new_registry(),
        crate::cron_jobs::CreateRequest {
            schedule: "@daily".into(),
            handler: "old".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let result = handler
        .update_cron_job(Parameters(UpdateInput {
            id: id.clone(),
            schedule: None,
            handler: Some("new".into()),
            metadata: None,
            enabled: None,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));

    crate::storage::remove_job_dir(&id).unwrap();
}

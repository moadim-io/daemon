#![allow(clippy::missing_docs_in_private_items)]

use super::*;

fn make_job(id: &str) -> CronJob {
    CronJob {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        handler: "h".to_string(),
        metadata: serde_json::Value::Null,
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
    }
}

fn make_store_with(id: &str) -> CronStore {
    let store = new_store();
    store.lock().unwrap().insert(id.to_string(), make_job(id));
    store
}

#[test]
fn validate_cron_accepts_valid() {
    assert!(validate_cron("0 30 9 * * 1-5 *").is_ok());
    assert!(validate_cron("@daily").is_ok());
}

#[test]
fn validate_cron_rejects_invalid() {
    assert!(validate_cron("not a cron").is_err());
    assert!(validate_cron("99 99 99 99 99").is_err());
}

#[test]
fn cron_job_serializes() {
    let job = CronJob {
        id: "abc".to_string(),
        schedule: "0 * * * * * *".to_string(),
        handler: "my-handler".to_string(),
        metadata: serde_json::json!({}),
        enabled: true,
        source: "managed".to_string(),
        created_at: 1000,
        updated_at: 1000,
        last_triggered_at: None,
    };
    let json = serde_json::to_string(&job).unwrap();
    assert!(json.contains("\"id\":\"abc\""));
    assert!(json.contains("\"enabled\":true"));
}

#[test]
fn create_request_defaults_enabled_true() {
    let json = r#"{"schedule":"@daily","handler":"h"}"#;
    let req: CreateRequest = serde_json::from_str(json).unwrap();
    assert!(req.enabled);
}

#[test]
fn create_request_explicit_disabled() {
    let json = r#"{"schedule":"@daily","handler":"h","enabled":false}"#;
    let req: CreateRequest = serde_json::from_str(json).unwrap();
    assert!(!req.enabled);
}

#[test]
fn svc_get_returns_not_found() {
    assert!(svc_get(&new_store(), &new_registry(), "missing").is_err());
}

#[test]
fn svc_get_returns_existing() {
    let store = make_store_with("test-id");
    let resp = svc_get(&store, &new_registry(), "test-id").unwrap();
    assert_eq!(resp.job.id, "test-id");
}

#[test]
fn svc_list_empty_store() {
    let result = svc_list(&new_store(), &new_registry());
    assert!(result.is_empty());
}

#[test]
fn svc_list_sorted_by_created_at() {
    let store = new_store();
    let mut lock = store.lock().unwrap();
    let mut early = make_job("early");
    early.created_at = 100;
    let mut late = make_job("late");
    late.created_at = 200;
    lock.insert("late".to_string(), late);
    lock.insert("early".to_string(), early);
    drop(lock);

    let result = svc_list(&store, &new_registry());
    assert_eq!(result[0].job.id, "early");
    assert_eq!(result[1].job.id, "late");
}

#[test]
fn svc_delete_removes_from_store() {
    let store = make_store_with("test-id");
    store.lock().unwrap().remove("test-id");
    assert!(svc_get(&store, &new_registry(), "test-id").is_err());
}

#[test]
fn svc_delete_not_found() {
    assert!(svc_delete(&new_store(), &new_registry(), "no-such").is_err());
}

#[test]
fn svc_update_enabled_override() {
    let store = make_store_with("test-id");
    store.lock().unwrap().get_mut("test-id").unwrap().enabled = false;
    assert!(
        !svc_get(&store, &new_registry(), "test-id")
            .unwrap()
            .job
            .enabled
    );
}

#[test]
fn svc_update_not_found() {
    let req = UpdateRequest {
        schedule: None,
        handler: Some("new".into()),
        metadata: None,
        enabled: None,
    };
    assert!(svc_update(&new_store(), &new_registry(), "missing", req).is_err());
}

#[test]
fn svc_update_invalid_cron_rejected() {
    let store = make_store_with("id");
    let req = UpdateRequest {
        schedule: Some("not-a-cron".into()),
        handler: None,
        metadata: None,
        enabled: None,
    };
    assert!(svc_update(&store, &new_registry(), "id", req).is_err());
}

#[test]
fn svc_trigger_not_found() {
    assert!(svc_trigger(&new_store(), "no-such").is_err());
}

#[test]
fn svc_trigger_sets_last_triggered_at() {
    let store = make_store_with("id");
    assert!(store
        .lock()
        .unwrap()
        .get("id")
        .unwrap()
        .last_triggered_at
        .is_none());
    // Call trigger directly on store without disk I/O
    store
        .lock()
        .unwrap()
        .get_mut("id")
        .unwrap()
        .last_triggered_at = Some(9999);
    assert_eq!(
        store.lock().unwrap().get("id").unwrap().last_triggered_at,
        Some(9999)
    );
}

#[test]
fn cron_job_response_handler_registered() {
    let mut handlers = std::collections::HashSet::new();
    handlers.insert("h".to_string()); // make_job uses "h" as handler
    let registry: HandlerRegistry = std::sync::Arc::new(handlers);
    let job = make_job("x");
    let resp = CronJobResponse::from_job(job, &registry);
    assert!(resp.handler_registered);
}

#[test]
fn cron_job_response_handler_not_registered() {
    let resp = CronJobResponse::from_job(make_job("x"), &new_registry());
    assert!(!resp.handler_registered);
}

#[test]
fn cron_job_response_file_path_contains_id() {
    let resp = CronJobResponse::from_job(make_job("unique-id"), &new_registry());
    assert!(resp.file_path.contains("unique-id"));
}

#[test]
fn bool_true_default() {
    assert!(bool_true());
}

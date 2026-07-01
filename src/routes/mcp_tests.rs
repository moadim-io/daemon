#![allow(clippy::missing_docs_in_private_items)]

use crate::cron_jobs::{new_registry, new_store};

use super::*;

fn make_handler() -> MoadimMcp {
    MoadimMcp::new(
        new_store(),
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    )
}

/// A throwaway shutdown signal for constructing a handler in tests; the `shutdown` tool fires it but
/// nothing awaits it, so notifying is a harmless no-op.
fn test_shutdown() -> crate::cron_jobs::ShutdownSignal {
    std::sync::Arc::new(tokio::sync::Notify::new())
}

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop. With no agent TOMLs present, agent validation falls back to the built-in names (so
/// `"claude"` is accepted) while `load_agent_command` finds no config — exercising the trigger
/// "no spawn" path without launching a real agent or writing into the user's real home. Tests in
/// this crate run single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> TempHome {
        let dir = std::env::temp_dir().join(format!("moadim-mcptest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        TempHome
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
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["status"], "ok");
    assert_eq!(val["running"], true);
    // Build provenance is surfaced for parity with `GET /health` and `--version`.
    assert_eq!(val["version"], crate::build_info::VERSION);
    assert_eq!(val["git_sha"], crate::build_info::GIT_SHA);
    assert_eq!(val["build_date"], crate::build_info::BUILD_DATE);
    // Resolved machine identity, for parity with `GET /health`.
    assert!(
        val["machine"].is_string() && !val["machine"].as_str().unwrap().is_empty(),
        "mcp health should carry a non-empty machine name, got: {val}"
    );
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
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["message"], "test-msg");
}

#[test]
fn list_cron_jobs_empty() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .list_cron_jobs(Parameters(LocalOnlyParam { local_only: None }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
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
    let handler = MoadimMcp::new(
        store,
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
    let req = crate::cron_jobs::CreateRequest {
        schedule: "not-a-cron".into(),
        handler: "h".into(),
        metadata: serde_json::Value::Null,
        machines: vec![crate::machine::current_machine()],
        enabled: true,
    };
    let result = handler.create_cron_job(Parameters(req)).unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn update_cron_job_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = MoadimMcp::new(
        crate::cron_jobs::new_store(),
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
    let result = handler
        .update_cron_job(Parameters(UpdateInput {
            id: "no-such".into(),
            schedule: None,
            handler: Some("h".into()),
            metadata: None,
            machines: None,
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
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();
    let handler = MoadimMcp::new(
        store,
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
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
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();
    let handler = MoadimMcp::new(
        store,
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
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
    let handler = MoadimMcp::new(
        store,
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
    let req = crate::cron_jobs::CreateRequest {
        schedule: "@daily".into(),
        handler: "mcp-handler".into(),
        metadata: serde_json::Value::Null,
        machines: vec![crate::machine::current_machine()],
        enabled: true,
    };
    let result = handler.create_cron_job(Parameters(req)).unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    let id = val["id"].as_str().unwrap().to_string();
    crate::storage::remove_job_dir(&id).unwrap();
}

#[test]
fn get_cron_job_tool_success() {
    use rmcp::handler::server::wrapper::Parameters;
    // The get tool reloads from disk first, so the job must be persisted to the (temp-home) jobs
    // dir; an in-memory-only insert would be wiped by the reload.
    let _home = TempHome::set();
    let job = crate::cron_jobs::CronJob {
        id: "get-test-id".into(),
        schedule: "@daily".into(),
        handler: "h".into(),
        metadata: serde_json::Value::Null,
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
    };
    crate::storage::write_job(&job).unwrap();
    let handler = MoadimMcp::new(
        crate::cron_jobs::new_store(),
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
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
    let handler = MoadimMcp::new(
        store.clone(),
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
    // Create a job first
    let created = crate::cron_jobs::svc_create(
        &store,
        &new_registry(),
        crate::cron_jobs::CreateRequest {
            schedule: "@daily".into(),
            handler: "old".into(),
            metadata: serde_json::Value::Null,
            machines: vec![crate::machine::current_machine()],
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
            machines: None,
            enabled: None,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));

    crate::storage::remove_job_dir(&id).unwrap();
}

// ── routine tools ──────────────────────────────────────────────────────────────

fn make_create_routine_req() -> crate::routines::CreateRoutineRequest {
    crate::routines::CreateRoutineRequest {
        schedule: "@daily".into(),
        title: "Mcp Routine".into(),
        agent: "claude".into(),
        prompt: "p".into(),
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
        .list_routines(Parameters(LocalOnlyParam { local_only: None }))
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
        new_store(),
        crate::paths::jobs_dir(),
        new_registry(),
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
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
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

    // update
    let result = handler
        .update_routine(Parameters(UpdateRoutineInput {
            id: id.clone(),
            schedule: None,
            title: Some("Renamed".into()),
            agent: None,
            prompt: None,
            repositories: None,
            machines: None,
            enabled: Some(false),
            ttl_secs: None,
            max_runtime_secs: None,
            tags: None,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));

    // trigger (records the manual trigger)
    let result = handler
        .trigger_routine(Parameters(IdInput { id: id.clone() }))
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
    let json_str = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(val["removed"].is_u64());
}

#[test]
fn update_routine_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .update_routine(Parameters(UpdateRoutineInput {
            id: "no-such".into(),
            schedule: None,
            title: Some("x".into()),
            agent: None,
            prompt: None,
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

// ── parity tools: agents / logs / shutdown ──────────────────────────────────────

#[test]
fn list_agents_tool_returns_array() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler.list_agents().unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(val.is_array());
}

#[test]
fn cron_job_logs_tool_returns_logs_for_existing_job() {
    use rmcp::handler::server::wrapper::Parameters;
    let store = crate::cron_jobs::new_store();
    let created = crate::cron_jobs::svc_create(
        &store,
        &new_registry(),
        crate::cron_jobs::CreateRequest {
            schedule: "@daily".into(),
            handler: "h".into(),
            metadata: serde_json::Value::Null,
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();
    let handler = MoadimMcp::new(
        store,
        crate::paths::jobs_dir(),
        new_registry(),
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
    let result = handler
        .cron_job_logs(Parameters(IdInput { id: id.clone() }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    // No log file has been produced yet, so the contents are empty (but the key is present).
    assert_eq!(val["logs"], "");
    crate::storage::remove_job_dir(&id).unwrap();
}

#[test]
fn cron_job_logs_tool_not_found_is_error() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .cron_job_logs(Parameters(IdInput {
            id: "no-such".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn routine_logs_tool_returns_logs_for_existing_routine() {
    use rmcp::handler::server::wrapper::Parameters;
    let _home = TempHome::set();
    let routines = crate::routines::new_store();
    let handler = MoadimMcp::new(
        new_store(),
        crate::paths::jobs_dir(),
        new_registry(),
        routines.clone(),
        crate::paths::routines_dir(),
        0,
        test_shutdown(),
    );
    let created = handler
        .create_routine(Parameters(make_create_routine_req()))
        .unwrap();
    let text = match &created.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
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
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
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
fn shutdown_tool_acknowledges() {
    let handler = make_handler();
    let result = handler.shutdown().unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["status"], "shutting down");
}

#[test]
fn restart_tool_spawns_helper_and_acknowledges() {
    // The tool spawns a detached `current_exe --background` helper; under the test harness that exe
    // is the test binary, which rejects `--background` and exits at once, so no real server starts.
    // TempHome keeps the helper's log file out of the real home.
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler.restart().unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["status"], "restarting");
    assert!(val["helper_pid"].as_u64().unwrap() > 0);
}

#[test]
fn get_lock_status_returns_unlocked_by_default() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler.get_lock_status().unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
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
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
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
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
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

#[test]
fn unlock_routines_all_removes_both_sentinels() {
    let _home = TempHome::set();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, true).unwrap();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, true).unwrap();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "all".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["locked"], false);
}

#[test]
fn unlock_routines_shared_removes_only_shared() {
    let _home = TempHome::set();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, true).unwrap();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["shared"], false);
}

#[test]
fn unlock_routines_local_removes_only_local() {
    let _home = TempHome::set();
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, true).unwrap();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "local".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(txt) => txt.text.clone(),
        _ => panic!("expected text content"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["local"], false);
}

#[test]
fn unlock_routines_unknown_scope_is_error() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "bad".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

/// A crontab shim for tests: accepts `-l` (prints empty) and `-` (swallows stdin), making
/// `sync_routines_to_crontab` succeed and exercising the fall-through path after the `if let Err`.
struct SucceedingCronShim {
    base: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl SucceedingCronShim {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("moadim-scshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store = base.join("store");
        std::fs::write(&store, "").unwrap();
        let store_display = store.to_string_lossy().into_owned();
        let script = base.join("crontab-ok.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then cat \"$STORE\"; elif [ \"$1\" = \"-\" ]; then cat > \"$STORE\"; fi\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { base, previous }
    }
}

impl Drop for SucceedingCronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(val) => std::env::set_var("MOADIM_CRONTAB_BIN", val),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// A crontab shim for tests: always exits non-zero so `sync_routines_to_crontab` returns `Err`,
/// exercising the `log::warn!` paths in `lock_routines` / `unlock_routines`.
struct FailingCronShim {
    base: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl FailingCronShim {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("moadim-fcshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let script = base.join("crontab-fail.sh");
        std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { base, previous }
    }
}

impl Drop for FailingCronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(val) => std::env::set_var("MOADIM_CRONTAB_BIN", val),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn lock_routines_succeeds_when_crontab_sync_passes() {
    // Covers the success fall-through `}` of `if let Err(sync_err) = sync_routines_to_crontab`.
    let _home = TempHome::set();
    let _shim = SucceedingCronShim::new();
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "local".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, false).unwrap();
}

#[test]
fn unlock_routines_succeeds_when_crontab_sync_passes() {
    // Covers the success fall-through `}` of `if let Err(sync_err) = sync_routines_to_crontab`.
    let _home = TempHome::set();
    let _shim = SucceedingCronShim::new();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "all".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn lock_routines_logs_warn_when_crontab_sync_fails() {
    // Covers the `log::warn!("crontab sync after lock failed: ...")` line.
    let _home = TempHome::set();
    let _shim = FailingCronShim::new();
    let handler = make_handler();
    // The lock still succeeds even if the subsequent crontab sync fails.
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, false).unwrap();
}

#[test]
fn unlock_routines_logs_warn_when_crontab_sync_fails() {
    // Covers the `log::warn!("crontab sync after unlock failed: ...")` line.
    let _home = TempHome::set();
    let _shim = FailingCronShim::new();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "all".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn lock_routines_returns_error_when_set_lock_fails() {
    // Covers the `return Ok(err(...))` on IO error path in lock_routines.
    // Make set_lock fail by placing a regular file where the config dir must be created.
    let dir = std::env::temp_dir().join(format!("moadim-lockfail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    // Write a file at `.config` so create_dir_all(".config/moadim") fails.
    std::fs::write(dir.join(".config"), b"not a dir").unwrap();
    // SAFETY: single-threaded.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
    // SAFETY: cleanup.
    unsafe {
        std::env::remove_var("MOADIM_HOME_OVERRIDE");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unlock_routines_returns_error_when_set_lock_fails() {
    // Covers the `return Ok(err(...))` IO error path in unlock_routines.
    // Create the sentinel path as a DIRECTORY instead of a file: `path.exists()` is true but
    // `std::fs::remove_file` returns EISDIR, triggering the error return.
    let dir = std::env::temp_dir().join(format!("moadim-unlockfail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join(".config").join("moadim")).unwrap();
    // SAFETY: single-threaded.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    // Make the shared sentinel path a directory so remove_file fails.
    let lock_path = crate::paths::global_lock_path();
    std::fs::create_dir_all(&lock_path).unwrap();

    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
    // SAFETY: cleanup.
    unsafe {
        std::env::remove_var("MOADIM_HOME_OVERRIDE");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

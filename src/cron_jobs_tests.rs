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
    assert!(validate_cron("30 9 * * 1-5").is_ok());
    assert!(validate_cron("@daily").is_ok());
    // legacy 7-field is still accepted
    assert!(validate_cron("0 30 9 * * 1-5 *").is_ok());
}

#[test]
fn validate_cron_accepts_5_field() {
    assert!(validate_cron("0 9 * * *").is_ok());
    assert!(validate_cron("30 9 * * 1-5").is_ok());
    assert!(validate_cron("*/15 * * * *").is_ok());
}

#[test]
fn validate_cron_rejects_invalid() {
    assert!(validate_cron("not a cron").is_err());
    assert!(validate_cron("99 99 99 99 99").is_err());
}

#[test]
fn validate_cron_accepts_all_documented_keywords() {
    for kw in [
        "@hourly",
        "@daily",
        "@weekly",
        "@monthly",
        "@yearly",
        "@annually",
    ] {
        assert!(validate_cron(kw).is_ok(), "{kw} should be accepted");
    }
}

#[test]
fn validate_cron_rejects_unsupported_keywords() {
    // @reboot and @midnight are documented as unsupported via the API.
    for kw in ["@reboot", "@midnight", "@nonsense"] {
        let err = validate_cron(kw);
        assert!(err.is_err(), "{kw} should be rejected");
        assert!(
            matches!(err, Err(AppError::BadRequest(_))),
            "{kw} should be rejected with BadRequest"
        );
    }
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

#[test]
fn svc_create_adds_to_store_and_disk() {
    let store = new_store();
    let req = CreateRequest {
        schedule: "@daily".into(),
        handler: "cov-handler".into(),
        metadata: serde_json::Value::Null,
        enabled: true,
    };
    let resp = svc_create(&store, &new_registry(), req).unwrap();
    assert!(!resp.job.id.is_empty());
    assert_eq!(resp.job.handler, "cov-handler");
    assert!(store.lock().unwrap().contains_key(&resp.job.id));
    assert!(crate::paths::job_toml_path(&resp.job.id).exists());
    crate::storage::remove_job_dir(&resp.job.id).unwrap();
}

#[test]
fn svc_create_invalid_cron_returns_err() {
    let store = new_store();
    let req = CreateRequest {
        schedule: "not-a-cron".into(),
        handler: "h".into(),
        metadata: serde_json::Value::Null,
        enabled: true,
    };
    assert!(svc_create(&store, &new_registry(), req).is_err());
}

#[test]
fn svc_update_changes_all_fields() {
    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "old".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let req = UpdateRequest {
        schedule: Some("@weekly".into()),
        handler: Some("new".into()),
        metadata: Some(serde_json::json!({"k": "v"})),
        enabled: Some(false),
    };
    let updated = svc_update(&store, &new_registry(), &id, req).unwrap();
    assert_eq!(updated.job.schedule, "@weekly");
    assert_eq!(updated.job.handler, "new");
    assert!(!updated.job.enabled);

    crate::storage::remove_job_dir(&id).unwrap();
}

#[test]
fn svc_delete_removes_from_store_and_disk() {
    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "h".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();
    let dir = crate::paths::job_dir(&id);
    assert!(dir.exists());

    svc_delete(&store, &new_registry(), &id).unwrap();
    assert!(!dir.exists());
    assert!(!store.lock().unwrap().contains_key(&id));
}

#[test]
fn svc_trigger_persists_last_triggered_at() {
    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "h".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();
    assert!(created.job.last_triggered_at.is_none());

    let triggered = svc_trigger(&store, &id).unwrap();
    assert!(triggered.last_triggered_at.is_some());

    crate::storage::remove_job_dir(&id).unwrap();
}

#[test]
fn from_ref_extracts_store_from_app_state() {
    use axum::extract::FromRef;
    let store = new_store();
    let state = AppState {
        store: store.clone(),
        handlers: new_registry(),
        routines: crate::routines::new_store(),
        uptime_start: 0,
        shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
    };
    let extracted = CronStore::from_ref(&state);
    // Same underlying Arc allocation
    assert!(std::sync::Arc::ptr_eq(&extracted, &store));
}

#[test]
fn schedule_description_present_for_valid_expression() {
    let resp = CronJobResponse::from_job(make_job("x"), &new_registry());
    // make_job uses "@daily"
    assert!(resp.schedule_description.is_some());
}

#[test]
fn schedule_description_none_for_unparseable_expression() {
    let mut job = make_job("x");
    job.schedule = "@reboot".to_string();
    let resp = CronJobResponse::from_job(job, &new_registry());
    assert!(resp.schedule_description.is_none());
}

#[test]
fn from_source_managed_maps_to_managed() {
    assert_eq!(
        CronJobSourceType::from_source("managed"),
        CronJobSourceType::Managed
    );
}

#[test]
fn from_source_non_managed_maps_to_system() {
    // Anything other than the exact "managed" string is treated as a read-only
    // OS-discovered (system) entry.
    assert_eq!(
        CronJobSourceType::from_source("system:user"),
        CronJobSourceType::System
    );
    assert_eq!(
        CronJobSourceType::from_source(""),
        CronJobSourceType::System
    );
}

#[test]
fn from_job_system_source_sets_source_type() {
    let mut job = make_job("sys");
    job.source = "system:root".to_string();
    let resp = CronJobResponse::from_job(job, &new_registry());
    assert_eq!(resp.source_type, CronJobSourceType::System);
}

/// Install a `MOADIM_CRONTAB_BIN` shim that always exits non-zero with a stderr that
/// does NOT contain "no crontab", so `read_crontab` (and thus `sync_to_crontab`) returns
/// an error. The previous value of the env var is restored on drop and the temp dir removed.
///
/// This exercises the best-effort `if let Err(err) = sync_to_crontab(..)` warn branches in
/// `svc_create`/`svc_update`/`svc_delete`: the crontab sync fails but the operation still succeeds.
struct FailingCrontabShim {
    /// Temp dir holding the shim script; removed on drop.
    base: std::path::PathBuf,
    /// Saved prior value of `MOADIM_CRONTAB_BIN` to restore on drop.
    previous: Option<std::ffi::OsString>,
}

impl FailingCrontabShim {
    fn install() -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt as _;

        let base = std::env::temp_dir().join(format!("moadim-cronfail-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let script = base.join("crontab-fail.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nif [ \"$1\" = \"-\" ]; then cat > /dev/null; fi\necho \"crontab boom\" 1>&2\nexit 1\n",
        )
        .unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); restored on drop.
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { base, previous }
    }
}

impl Drop for FailingCrontabShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test harness; restore the saved value.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn svc_create_succeeds_despite_crontab_sync_failure() {
    let _shim = FailingCrontabShim::install();
    let store = new_store();
    let resp = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "sync-fail-create".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .expect("create must succeed even when crontab sync fails");
    assert!(store.lock().unwrap().contains_key(&resp.job.id));
    crate::storage::remove_job_dir(&resp.job.id).unwrap();
}

#[test]
fn svc_update_succeeds_despite_crontab_sync_failure() {
    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "sync-fail-update".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let _shim = FailingCrontabShim::install();
    let updated = svc_update(
        &store,
        &new_registry(),
        &id,
        UpdateRequest {
            schedule: Some("@weekly".into()),
            handler: None,
            metadata: None,
            enabled: None,
        },
    )
    .expect("update must succeed even when crontab sync fails");
    assert_eq!(updated.job.schedule, "@weekly");
    crate::storage::remove_job_dir(&id).unwrap();
}

#[test]
fn svc_delete_succeeds_despite_crontab_sync_failure() {
    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "sync-fail-delete".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let _shim = FailingCrontabShim::install();
    svc_delete(&store, &new_registry(), &id)
        .expect("delete must succeed even when crontab sync fails");
    assert!(!store.lock().unwrap().contains_key(&id));
}

#[test]
fn svc_trigger_logs_when_handler_spawn_fails() {
    // A handler file that exists but is not executable cannot be spawned: `Command::spawn`
    // returns Err, exercising the spawn-failure `log::warn!` branch. The trigger itself
    // still succeeds (best-effort spawn).
    let handlers = crate::paths::handlers_dir();
    std::fs::create_dir_all(&handlers).unwrap();
    let handler_name = format!("cov-nonexec-{}", uuid::Uuid::new_v4());
    let handler_path = handlers.join(&handler_name);
    // No shebang, no execute bit → spawn fails with a permission/exec error.
    std::fs::write(&handler_path, "not an executable").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&handler_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: handler_name.clone(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let triggered = svc_trigger(&store, &id).expect("trigger succeeds despite spawn failure");
    assert!(triggered.last_triggered_at.is_some());

    crate::storage::remove_job_dir(&id).unwrap();
    let _ = std::fs::remove_file(&handler_path);
}

#[test]
fn svc_trigger_spawns_existing_handler_script() {
    // Place an executable handler script under the handlers dir so svc_trigger's
    // `handler_path.exists()` branch is taken and the script is spawned.
    let handlers = crate::paths::handlers_dir();
    std::fs::create_dir_all(&handlers).unwrap();
    let handler_name = format!("cov-handler-{}", uuid::Uuid::new_v4());
    let handler_path = handlers.join(&handler_name);
    std::fs::write(&handler_path, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&handler_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&handler_path, perms).unwrap();
    }

    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: handler_name.clone(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let triggered = svc_trigger(&store, &id).unwrap();
    assert!(triggered.last_triggered_at.is_some());

    crate::storage::remove_job_dir(&id).unwrap();
    let _ = std::fs::remove_file(&handler_path);
}

#[tokio::test]
async fn replace_handler_updates_job() {
    use axum::extract::{Path, State};
    use axum::Json;

    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "before".into(),
            metadata: serde_json::Value::Null,
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let state = AppState {
        store: store.clone(),
        handlers: new_registry(),
        routines: crate::routines::new_store(),
        uptime_start: 0,
        shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
    };
    let body = UpdateRequest {
        schedule: None,
        handler: Some("after".into()),
        metadata: None,
        enabled: None,
    };
    let resp = replace(State(state), Path(id.clone()), Json(body))
        .await
        .unwrap();
    assert_eq!(resp.0.job.handler, "after");

    crate::storage::remove_job_dir(&id).unwrap();
}

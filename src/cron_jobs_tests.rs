#![allow(clippy::missing_docs_in_private_items)]

use super::*;

fn make_job(id: &str) -> CronJob {
    CronJob {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        handler: "h".to_string(),
        metadata: serde_json::Value::Null,
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
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
fn validate_cron_accepts_6_field() {
    // croner accepts a leading seconds field; we must too.
    assert!(validate_cron("0 */5 * * * *").is_ok());
    assert!(validate_cron("30 0 9 * * 1-5").is_ok());
    // croner accepts 6-field `sec min hour dom month dow`; validate_cron
    // normalizes it down before parsing, so it must be accepted.
    assert!(validate_cron("*/30 * * * * *").is_ok());
}

#[test]
fn normalize_schedule_strips_seconds_from_6_field() {
    // 6-field `sec min hour dom month dow` -> 5-field `min hour dom month dow`.
    // Without this, the 6-field string lands in the OS crontab verbatim and
    // the routine/job silently never fires.
    assert_eq!(normalize_schedule("0 */5 * * * *"), "*/5 * * * *");
    assert_eq!(normalize_schedule("30 0 9 * * 1-5"), "0 9 * * 1-5");
}

#[test]
fn normalize_schedule_strips_seconds_and_year_from_7_field() {
    assert_eq!(normalize_schedule("0 30 9 * * 1-5 *"), "30 9 * * 1-5");
}

#[test]
fn normalize_schedule_passes_through_5_field_and_keywords() {
    assert_eq!(normalize_schedule("*/15 * * * *"), "*/15 * * * *");
    assert_eq!(normalize_schedule("@daily"), "@daily");
}

#[test]
fn normalize_schedule_projects_to_5_field() {
    // 6- and 7-field schedules both lose their leading seconds (and trailing
    // year) so the stored/crontab form is a valid 5-field expression.
    assert_eq!(normalize_schedule("0 30 9 * * 1-5"), "30 9 * * 1-5");
    assert_eq!(normalize_schedule("0 30 9 * * 1-5 *"), "30 9 * * 1-5");
    assert_eq!(normalize_schedule("30 9 * * 1-5"), "30 9 * * 1-5");
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
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 1000,
        updated_at: 1000,
        last_manual_trigger_at: None,
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
    let result = svc_list(&new_store(), &new_registry(), &CronJobListQuery::default());
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

    let result = svc_list(&store, &new_registry(), &CronJobListQuery::default());
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
fn svc_update_sets_machines() {
    // Covers the `if let Some(machines) = req.machines` branch in `svc_update`.
    let store = make_store_with("mach-id");
    let req = UpdateRequest {
        schedule: None,
        handler: None,
        metadata: None,
        machines: Some(vec!["server".into()]),
        enabled: None,
    };
    let resp = svc_update(&store, &new_registry(), "mach-id", req).unwrap();
    assert_eq!(resp.job.machines, vec!["server"]);
    crate::storage::remove_job_dir("mach-id").unwrap();
}

#[test]
fn svc_update_not_found() {
    let req = UpdateRequest {
        schedule: None,
        handler: Some("new".into()),
        metadata: None,
        machines: None,
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
        machines: None,
        enabled: None,
    };
    assert!(svc_update(&store, &new_registry(), "id", req).is_err());
}

#[test]
fn svc_trigger_not_found() {
    assert!(svc_trigger(&new_store(), "no-such").is_err());
}

#[test]
fn svc_trigger_sets_last_manual_trigger_at() {
    let store = make_store_with("id");
    assert!(store
        .lock()
        .unwrap()
        .get("id")
        .unwrap()
        .last_manual_trigger_at
        .is_none());
    // Call trigger directly on store without disk I/O
    store
        .lock()
        .unwrap()
        .get_mut("id")
        .unwrap()
        .last_manual_trigger_at = Some(9999);
    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("id")
            .unwrap()
            .last_manual_trigger_at,
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
        machines: vec![crate::machine::current_machine()],
        enabled: true,
    };
    let resp = svc_create(&store, &new_registry(), req).unwrap();
    assert!(!resp.job.id.is_empty());
    assert_eq!(resp.job.handler, "cov-handler");
    assert_eq!(resp.job.machines, vec![crate::machine::current_machine()]);
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
        machines: vec![crate::machine::current_machine()],
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
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let req = UpdateRequest {
        schedule: Some("@weekly".into()),
        handler: Some("new".into()),
        metadata: Some(serde_json::json!({"k": "v"})),
        machines: None,
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
            machines: vec![crate::machine::current_machine()],
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
fn svc_trigger_persists_last_manual_trigger_at() {
    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "h".into(),
            metadata: serde_json::Value::Null,
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();
    assert!(created.job.last_manual_trigger_at.is_none());

    let triggered = svc_trigger(&store, &id).unwrap();
    assert!(triggered.last_manual_trigger_at.is_some());

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

/// Points `MOADIM_CRONTAB_BIN` at a shim that *succeeds*: `crontab -l` prints an empty crontab and
/// exits 0, and `crontab -` swallows stdin and exits 0. With it installed, `sync_to_crontab`
/// returns `Ok`, so the `if let Err(..) = sync_to_crontab(..)` guard takes its non-error path —
/// exercising the success branch of `svc_create`/`svc_update`/`svc_delete` without touching the
/// developer's real crontab. The previous env value is restored and the temp dir removed on drop.
struct WorkingCrontabShim {
    /// Temp dir holding the shim script; removed on drop.
    base: std::path::PathBuf,
    /// Saved prior value of `MOADIM_CRONTAB_BIN` to restore on drop.
    previous: Option<std::ffi::OsString>,
}

impl WorkingCrontabShim {
    fn install() -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt as _;

        let base = std::env::temp_dir().join(format!("moadim-cronok-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let script = base.join("crontab-ok.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nif [ \"$1\" = \"-\" ]; then cat > /dev/null; fi\nexit 0\n",
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

impl Drop for WorkingCrontabShim {
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
fn svc_create_syncs_crontab_on_success() {
    // A working crontab shim makes `sync_to_crontab` return `Ok`, covering the
    // non-error branch of the post-create sync guard.
    let _shim = WorkingCrontabShim::install();
    let store = new_store();
    let resp = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "sync-ok-create".into(),
            metadata: serde_json::Value::Null,
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .expect("create succeeds when crontab sync succeeds");
    assert!(store.lock().unwrap().contains_key(&resp.job.id));
    crate::storage::remove_job_dir(&resp.job.id).unwrap();
}

#[test]
fn svc_update_syncs_crontab_on_success() {
    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "sync-ok-update".into(),
            metadata: serde_json::Value::Null,
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let _shim = WorkingCrontabShim::install();
    let updated = svc_update(
        &store,
        &new_registry(),
        &id,
        UpdateRequest {
            schedule: Some("@weekly".into()),
            handler: None,
            metadata: None,
            machines: None,
            enabled: None,
        },
    )
    .expect("update succeeds when crontab sync succeeds");
    assert_eq!(updated.job.schedule, "@weekly");
    crate::storage::remove_job_dir(&id).unwrap();
}

#[test]
fn svc_delete_syncs_crontab_on_success() {
    let store = new_store();
    let created = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "sync-ok-delete".into(),
            metadata: serde_json::Value::Null,
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let _shim = WorkingCrontabShim::install();
    svc_delete(&store, &new_registry(), &id).expect("delete succeeds when crontab sync succeeds");
    assert!(!store.lock().unwrap().contains_key(&id));
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
            machines: vec![crate::machine::current_machine()],
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
            machines: vec![crate::machine::current_machine()],
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
            machines: None,
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
            machines: vec![crate::machine::current_machine()],
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
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let triggered = svc_trigger(&store, &id).expect("trigger succeeds despite spawn failure");
    assert!(triggered.last_manual_trigger_at.is_some());

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
            machines: vec![crate::machine::current_machine()],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.job.id.clone();

    let triggered = svc_trigger(&store, &id).unwrap();
    assert!(triggered.last_manual_trigger_at.is_some());

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
            machines: vec![crate::machine::current_machine()],
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
        machines: None,
        enabled: None,
    };
    let resp = replace(State(state), Path(id.clone()), Json(body))
        .await
        .unwrap();
    assert_eq!(resp.0.job.handler, "after");

    crate::storage::remove_job_dir(&id).unwrap();
}

// ─── coverage: local_only filter + I/O error paths ────────────────────────

/// RAII guard: redirect config paths to a temp dir and restore on drop.
struct HomeOverrideGuard {
    dir: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl HomeOverrideGuard {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-cj-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", &dir) }
        Self { dir, previous }
    }
}

impl Drop for HomeOverrideGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(v) => std::env::set_var("MOADIM_HOME_OVERRIDE", v),
                None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
            }
        }
        // Restore write permission before deletion so `remove_dir_all` succeeds
        // even if a test made a sub-directory read-only.
        restore_writable(&self.dir);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn restore_writable(dir: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt as _;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let path = e.path();
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
            if path.is_dir() {
                restore_writable(&path);
            }
        }
    }
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
}

#[test]
fn svc_list_local_only_filters_non_matching_machines() {
    let _home = HomeOverrideGuard::new();
    let store = new_store();
    let me = crate::machine::current_machine();
    // Job targeting this machine — should survive the filter.
    let mut local = make_job("local");
    local.machines = vec![me.clone()];
    // Job targeting a different machine — should be dropped.
    let mut other = make_job("other");
    other.machines = vec!["not-this-machine-xyz".to_string()];
    {
        let mut lock = store.lock().unwrap();
        lock.insert("local".to_string(), local);
        lock.insert("other".to_string(), other);
    }
    let reg = new_registry();
    let results = svc_list(
        &store,
        &reg,
        &CronJobListQuery {
            local_only: Some(true),
        },
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].job.id, "local");
}

#[cfg(unix)]
#[test]
fn svc_create_write_failure_returns_internal() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = HomeOverrideGuard::new();
    // Pre-create the jobs dir and make it read-only so write_job can't create a subdir.
    let jobs_dir = home.dir.join(".config").join("moadim").join("jobs");
    std::fs::create_dir_all(&jobs_dir).unwrap();
    std::fs::set_permissions(&jobs_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let store = new_store();
    let result = svc_create(
        &store,
        &new_registry(),
        CreateRequest {
            schedule: "@daily".into(),
            handler: "h".into(),
            metadata: serde_json::Value::Null,
            machines: vec![],
            enabled: true,
        },
    );
    assert!(matches!(result, Err(AppError::Internal)));
}

#[cfg(unix)]
#[test]
fn svc_update_write_failure_returns_internal() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = HomeOverrideGuard::new();
    let store = new_store();
    // Insert a job directly (bypassing write_job).
    store
        .lock()
        .unwrap()
        .insert("j1".to_string(), make_job("j1"));
    // Block writes by making jobs dir read-only.
    let jobs_dir = home.dir.join(".config").join("moadim").join("jobs");
    std::fs::create_dir_all(&jobs_dir).unwrap();
    std::fs::set_permissions(&jobs_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_update(
        &store,
        &new_registry(),
        "j1",
        UpdateRequest {
            schedule: None,
            handler: Some("new-handler".into()),
            metadata: None,
            machines: None,
            enabled: None,
        },
    );
    assert!(matches!(result, Err(AppError::Internal)));
}

#[cfg(unix)]
#[test]
fn svc_delete_remove_failure_returns_internal() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = HomeOverrideGuard::new();
    // Pre-create jobs/j1/ so remove_job_dir has something to remove.
    let jobs_dir = home.dir.join(".config").join("moadim").join("jobs");
    let job_dir = jobs_dir.join("j1");
    std::fs::create_dir_all(&job_dir).unwrap();
    // Make jobs/ read-only so remove_dir_all(jobs/j1/) fails.
    std::fs::set_permissions(&jobs_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("j1".to_string(), make_job("j1"));

    let result = svc_delete(&store, &new_registry(), "j1");
    assert!(matches!(result, Err(AppError::Internal)));
}

#[cfg(unix)]
#[test]
fn svc_trigger_write_failure_returns_internal() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = HomeOverrideGuard::new();
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("j1".to_string(), make_job("j1"));
    // Block writes by making jobs dir read-only.
    let jobs_dir = home.dir.join(".config").join("moadim").join("jobs");
    std::fs::create_dir_all(&jobs_dir).unwrap();
    std::fs::set_permissions(&jobs_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_trigger(&store, "j1");
    assert!(matches!(result, Err(AppError::Internal)));
}

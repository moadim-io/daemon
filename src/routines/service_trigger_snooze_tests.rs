#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;
use std::sync::Mutex;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// Serializes the tests that swap `MOADIM_CRONTAB_BIN`, so concurrent service tests
/// never see a partially-restored value. The poisoned-lock case is recovered into the guard.
static PATH_GUARD: Mutex<()> = Mutex::new(());

fn with_working_crontab(body: impl FnOnce()) {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = std::env::temp_dir().join(format!("moadim-routcronok-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let script = base.join("crontab-ok.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nif [ \"$1\" = \"-\" ]; then cat > /dev/null; fi\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let saved = std::env::var_os("MOADIM_CRONTAB_BIN");
    std::env::set_var("MOADIM_CRONTAB_BIN", &script);
    body();
    match saved {
        Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
        None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
    }
    let _ = std::fs::remove_dir_all(&base);
    drop(guard);
}

#[test]
fn svc_snooze_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_snooze(&new_store(), "nope", Some(1), None),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_snooze_rejects_both_modes_set() {
    let _home = TempHome::set();
    let store = new_store();
    let routine = make_routine("snooze-both-id", "Snooze Both ZZZ", 1, 1);
    store
        .lock()
        .unwrap()
        .insert("snooze-both-id".into(), routine);

    let result = svc_snooze(&store, "snooze-both-id", Some(1), Some(1));
    assert!(
        matches!(result, Err(AppError::BadRequest(_))),
        "expected BadRequest, got {result:?}"
    );
}

#[test]
fn svc_snooze_sets_and_clears() {
    let _home = TempHome::set();
    let store = new_store();
    let routine = make_routine("snooze-set-clear-id", "Snooze Set Clear ZZZ", 1, 1);
    store
        .lock()
        .unwrap()
        .insert("snooze-set-clear-id".into(), routine);

    let snoozed = svc_snooze(&store, "snooze-set-clear-id", Some(999), None).unwrap();
    assert_eq!(snoozed.snoozed_until, Some(999));
    assert_eq!(snoozed.skip_runs, None);
    assert_eq!(
        crate::routine_storage::load_store()
            .lock()
            .unwrap()
            .get("snooze-set-clear-id")
            .map(|routine| routine.snoozed_until),
        Some(Some(999)),
        "svc_snooze must persist to disk, not just the in-memory store"
    );

    let cleared = svc_snooze(&store, "snooze-set-clear-id", None, None).unwrap();
    assert_eq!(cleared.snoozed_until, None);
    assert_eq!(cleared.skip_runs, None);
}

#[cfg(unix)]
#[test]
fn svc_snooze_returns_internal_on_write_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L663: `write_routine(..).map_err(|_| AppError::Internal)?` in `svc_snooze`.
    let _home = TempHome::set();
    let title = "Svc Snooze Write Fail ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let routine = make_routine("snooze-write-fail-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("snooze-write-fail-id".into(), routine);

    let dir = crate::paths::routine_dir(&slug);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_snooze(&store, "snooze-write-fail-id", Some(999), None);

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[test]
fn svc_trigger_returns_locked_when_globally_locked() {
    let _home = TempHome::set();
    let lock_path = crate::paths::global_lock_path();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&lock_path, b"").unwrap();

    let store = new_store();
    let routine = make_routine("lock-trig-id", "Lock Trigger Test ZZZ", 1, 1);
    store.lock().unwrap().insert("lock-trig-id".into(), routine);

    let result = svc_trigger(&store, "lock-trig-id");
    assert!(
        matches!(result, Err(AppError::Locked(_))),
        "expected Locked error, got {result:?}"
    );
}

#[test]
fn svc_trigger_scheduled_returns_locked_when_globally_locked() {
    let _home = TempHome::set();
    let lock_path = crate::paths::global_local_lock_path();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&lock_path, b"").unwrap();

    let store = new_store();
    let routine = make_routine("lock-sched-id", "Lock Sched Test ZZZ", 1, 1);
    store
        .lock()
        .unwrap()
        .insert("lock-sched-id".into(), routine);

    let result = svc_trigger_scheduled(&store, "lock-sched-id");
    assert!(
        matches!(result, Err(AppError::Locked(_))),
        "expected Locked error, got {result:?}"
    );
}

#[cfg(unix)]
#[test]
fn svc_trigger_returns_internal_on_write_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L433: `write_routine(..).map_err(|_| AppError::Internal)?` in `svc_trigger`.
    // After updating `last_manual_trigger_at` in memory, write_routine is called; it fails
    // because the slug dir is read-only.
    let _home = TempHome::set();
    let title = "Svc Trigger Write Fail ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let routine = make_routine("trig-write-fail-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("trig-write-fail-id".into(), routine);

    let dir = crate::paths::routine_dir(&slug);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_trigger(&store, "trig-write-fail-id");

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[test]
fn svc_create_syncs_crontab_on_success() {
    let _home = TempHome::set();
    // A working crontab shim makes the post-create sync return `Ok`, covering the
    // non-error branch of the sync guard in `svc_create`.
    let title = "Svc Create Sync OK ZZZ";
    let store = new_store();
    with_working_crontab(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                model: None,
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                prompt: "p".into(),
                goal: None,
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: vec![],
            },
        )
        .unwrap();
        assert_eq!(created.routine.title, title);
    });
}

#[test]
fn svc_update_syncs_crontab_on_success() {
    let _home = TempHome::set();
    let title = "Svc Update Sync OK ZZZ";
    let store = new_store();
    let routine = make_routine("upd-sync-ok-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("upd-sync-ok-id".into(), routine);
    with_working_crontab(|| {
        let updated = svc_update(
            &store,
            "upd-sync-ok-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                title: None,
                agent: None,
                prompt: Some("changed".into()),
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.prompt, "changed");
    });
}

#[test]
fn svc_delete_syncs_crontab_on_success() {
    let _home = TempHome::set();
    let title = "Svc Delete Sync OK ZZZ";
    let store = new_store();
    let routine = make_routine("del-sync-ok-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("del-sync-ok-id".into(), routine);
    with_working_crontab(|| {
        let deleted = svc_delete(&store, "del-sync-ok-id").unwrap();
        assert_eq!(deleted.routine.title, title);
    });
}

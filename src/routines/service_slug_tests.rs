#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::{new_store, slugify};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing the
/// env var and the temp dir on drop. This keeps `svc_create`/`svc_update`/`write_routine` and the
/// other disk-touching paths off the developer's real `~/.moadim`, so a panicking assertion can never
/// leak test routines into the real home. Tests in this crate run single-threaded
/// (`RUST_TEST_THREADS=1`), so the global env mutation is safe.
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

/// Build a routine with overridable identity, title, timestamps, and repository URL.
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
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// Wrap a list of routines into a populated [`RoutineStore`].
fn store_with(routines: Vec<Routine>) -> RoutineStore {
    let mut map = HashMap::new();
    for routine in routines {
        map.insert(routine.id.clone(), routine);
    }
    Arc::new(Mutex::new(map))
}

/// Build a minimal valid create request; callers tweak the field under test.
fn valid_create_request() -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
        title: "Valid Title".into(),
        agent: "claude".into(),
        prompt: "do the thing".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
    }
}

/// Build a no-op update request (every field `None`); callers set one field.
fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        model: None,
        schedule: None,
        title: None,
        agent: None,
        prompt: None,
        goal: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: None,
    }
}

/// Serializes the tests that clear `PATH`, so concurrent service tests never see
/// a stripped environment. The poisoned-lock case is recovered into the guard.
static PATH_GUARD: Mutex<()> = Mutex::new(());

/// Run `body` with an empty `PATH`, restoring the original value afterwards.
///
/// Clearing `PATH` makes the `crontab` and `sh` lookups inside the crontab sync
/// and the trigger spawn fail to launch, exercising their warning branches.
fn with_empty_path(body: impl FnOnce()) {
    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    body();
    match saved {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(guard);
}

#[test]
fn svc_create_rejects_duplicate_slug() {
    let _home = TempHome::set();
    // Covers the slug-conflict branch in `svc_create`: an existing routine whose
    // title slugifies to the same value forces a `Conflict`.
    let title = "Svc Create Dup ZZZ";
    let store = new_store();
    // `with_empty_path` so the post-create/delete crontab sync cannot spawn the
    // real `crontab` binary and clobber the developer's live crontab (issue #175).
    with_empty_path(|| {
        let first = svc_create(
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

        let conflict = svc_create(
            &store,
            CreateRoutineRequest {
                model: None,
                schedule: "@daily".into(),
                // Different casing/spacing, same slug.
                title: "  svc create   DUP zzz ".into(),
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
        );
        assert!(matches!(conflict, Err(AppError::Conflict(_))));

        svc_delete(&store, &first.routine.id).unwrap();
    });
}

#[test]
fn svc_create_trims_title_before_persisting() {
    // Covers the title `.trim()` on the `svc_create` store path: a padded title is
    // length-checked trimmed but must also be *stored* trimmed, so the disclosure /
    // iCal SUMMARY / UI rows never render the surrounding whitespace.
    let title = "Svc Create Trim ZZZ";
    let store = new_store();
    with_empty_path(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                title: "   Svc Create Trim ZZZ   ".into(),
                ..valid_create_request()
            },
        )
        .unwrap();
        assert_eq!(created.routine.title, title);
        svc_delete(&store, &created.routine.id).unwrap();
    });
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_create_rejects_malformed_agent_config() {
    let _home = TempHome::set();
    // A referenced agent whose `<name>.toml` is present but unparseable is rejected at create time
    // with `BadRequest` quoting the parse error — not silently skipped at fire time.
    let agent_name = "svc-create-malformed-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = [\n").unwrap();

    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            title: "Svc Create Malformed ZZZ".into(),
            agent: agent_name.into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("malformed config")),
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[test]
fn svc_create_rejects_unreadable_agent_config() {
    // A referenced agent whose `<name>.toml` is present but unreadable (here a directory at the
    // path, which reads back as a non-`NotFound` I/O error) is rejected at create time with
    // `BadRequest` — not accepted and left as a green-dot routine that never fires.
    let agent_name = "svc-create-unreadable-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::create_dir_all(&cfg).unwrap();

    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            title: "Svc Create Unreadable ZZZ".into(),
            agent: agent_name.into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![],
            machines: vec![],
            tags: vec![],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("unreadable config")),
        other => panic!("expected BadRequest, got {other:?}"),
    }

    std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn svc_update_rejects_malformed_agent_config() {
    let _home = TempHome::set();
    // The same rejection applies when an update switches a routine to a malformed agent.
    let agent_name = "svc-update-malformed-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = [\n").unwrap();

    let title = "Svc Update Malformed ZZZ";
    let store = new_store();
    let routine = make_routine("upd-mal-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-mal-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-mal-id",
        UpdateRoutineRequest {
            model: None,
            schedule: None,
            title: None,
            agent: Some(agent_name.into()),
            prompt: None,
            goal: None,
            repositories: None,
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: None,
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("malformed config")),
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[test]
fn svc_update_rejects_renaming_into_existing_slug() {
    let _home = TempHome::set();
    // Covers the slug-conflict branch in `svc_update`: renaming one routine to a
    // title that another routine already owns yields a `Conflict`.
    let title_keep = "Svc Update Keep ZZZ";
    let title_other = "Svc Update Other ZZZ";
    // Build a store directly so both routines coexist before the rename attempt.
    let store = new_store();
    let routine_keep = make_routine("keep-id", title_keep, 1, 1);
    let routine_other = make_routine("other-id", title_other, 2, 2);
    crate::routine_storage::write_routine(&routine_keep).unwrap();
    crate::routine_storage::write_routine(&routine_other).unwrap();
    store.lock().unwrap().insert("keep-id".into(), routine_keep);
    store
        .lock()
        .unwrap()
        .insert("other-id".into(), routine_other);

    // Wrapped defensively: the rename short-circuits on `Conflict` before the
    // sync, but `with_empty_path` guarantees no real crontab write either way (#175).
    with_empty_path(|| {
        let conflict = svc_update(
            &store,
            "other-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                // Rename "other" into the slug already owned by "keep".
                title: Some(title_keep.into()),
                agent: None,
                prompt: None,
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
            },
        );
        assert!(matches!(conflict, Err(AppError::Conflict(_))));
    });
}

#[test]
fn svc_update_migrates_workbenches_on_rename() {
    let _home = TempHome::set();
    // Covers #267: renaming a routine must not strand its prior workbenches under the old
    // slug, or `svc_logs` and the cleanup watchdog silently lose track of them.
    let old_title = "Svc Update Rename Old ZZZ";
    let new_title = "Svc Update Rename New ZZZ";
    let old_slug = slugify(old_title);
    let new_slug = slugify(new_title);
    let store = store_with(vec![make_routine("rename-id", old_title, 1, 1)]);

    let workbenches = crate::paths::workbenches_dir();
    let old_dir = workbenches.join(format!("{old_slug}-1000"));
    std::fs::create_dir_all(&old_dir).unwrap();
    std::fs::write(old_dir.join("agent.log"), "prior run log").unwrap();

    // An unparseable directory name alongside it: skipped by `parse_workbench_name`, left
    // untouched by the migration.
    let unparseable = workbenches.join("not-a-workbench-name");
    std::fs::create_dir_all(&unparseable).unwrap();

    // A second old-slug workbench (older than the one above, so it never wins "newest") whose
    // destination is already occupied by a *non-empty* directory, so `std::fs::rename` fails for
    // it: covers the best-effort warn-and-skip branch. The source is left in place rather than
    // silently dropped.
    let blocked_old = workbenches.join(format!("{old_slug}-500"));
    std::fs::create_dir_all(&blocked_old).unwrap();
    let blocked_new = workbenches.join(format!("{new_slug}-500"));
    std::fs::create_dir_all(&blocked_new).unwrap();
    std::fs::write(blocked_new.join("marker"), "occupied").unwrap();

    with_empty_path(|| {
        svc_update(
            &store,
            "rename-id",
            UpdateRoutineRequest {
                title: Some(new_title.into()),
                ..empty_update_request()
            },
        )
        .unwrap();
    });

    // The old-slug workbench is gone and its content now lives under the new slug, keyed by
    // the same trigger timestamp.
    assert!(!old_dir.exists());
    let migrated = workbenches.join(format!("{new_slug}-1000"));
    assert_eq!(
        std::fs::read_to_string(migrated.join("agent.log")).unwrap(),
        "prior run log"
    );

    // The unparseable directory is untouched.
    assert!(unparseable.exists());

    // The blocked rename left the source in place and the occupied destination unchanged.
    assert!(blocked_old.exists());
    assert_eq!(
        std::fs::read_to_string(blocked_new.join("marker")).unwrap(),
        "occupied"
    );

    // `svc_logs` (which looks up by the *current* slug) can still find the newest migrated run.
    let logs = svc_logs(&store, "rename-id").unwrap();
    assert_eq!(logs, "prior run log");
}

#[test]
fn svc_update_sets_ttl_secs() {
    let _home = TempHome::set();
    // Covers the `req.ttl_secs` apply branch in `svc_update`.
    let title = "Svc Update Ttl ZZZ";
    let store = new_store();
    let routine = make_routine("ttl-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("ttl-id".into(), routine);

    // `with_empty_path` keeps the post-update crontab sync from touching the real
    // crontab (issue #175): the update succeeds, the sync just warns.
    // 1800 < the @daily routine's ttl ceiling (min(MAX_TTL_SECS=3600, interval)), so it is a value
    // that is actually in force rather than one silently clamped down (#468).
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "ttl-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                title: None,
                agent: None,
                prompt: None,
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: Some(1800),
                max_runtime_secs: None,
                tags: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.ttl_secs, Some(1800));
    });
}

#[test]
fn svc_update_sets_max_runtime_secs() {
    let _home = TempHome::set();
    // Covers the `req.max_runtime_secs` apply branch in `svc_update`.
    let title = "Svc Update Max Runtime ZZZ";
    let store = new_store();
    let routine = make_routine("max-runtime-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("max-runtime-id".into(), routine);

    // `with_empty_path` keeps the post-update crontab sync from touching the real
    // crontab (issue #175): the update succeeds, the sync just warns.
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "max-runtime-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                title: None,
                agent: None,
                prompt: None,
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: Some(1234),
                tags: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.max_runtime_secs, Some(1234));
    });
}

#[test]
fn svc_update_trims_title_before_persisting() {
    // Covers the title `.trim()` on the `svc_update` apply path. Renaming with the
    // same slug but different spacing/case must store the trimmed title.
    let title = "Svc Update Trim ZZZ";
    let store = new_store();
    let routine = make_routine("trim-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("trim-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "trim-id",
            UpdateRoutineRequest {
                // Same slug, padded: applies the rename branch without a conflict.
                title: Some("  Svc Update Trim ZZZ  ".into()),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.title, title);
    });

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

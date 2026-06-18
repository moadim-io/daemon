#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use crate::routines::{new_store, slugify, Repository};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Build a routine with overridable identity, title, timestamps, and repository URL.
fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        repositories: vec![],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_triggered_at: None,
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

#[test]
fn svc_list_sorts_by_updated_at() {
    // Covers the `RoutineSort::Updated` arm: sort by `updated_at` ascending.
    let store = store_with(vec![
        make_routine("late", "Zeta", 100, 300),
        make_routine("early", "Alpha", 100, 100),
        make_routine("mid", "Mid", 100, 200),
    ]);
    let query = RoutineListQuery {
        sort: RoutineSort::Updated,
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list[0].routine.id, "early");
    assert_eq!(list[1].routine.id, "mid");
    assert_eq!(list[2].routine.id, "late");
}

#[test]
fn svc_list_sorts_by_title_case_insensitively() {
    // Covers the `RoutineSort::Title` arm: sort by lowercased title.
    let store = store_with(vec![
        make_routine("banana", "banana", 0, 0),
        make_routine("apple", "Apple", 0, 0),
        make_routine("cherry", "CHERRY", 0, 0),
    ]);
    let query = RoutineListQuery {
        sort: RoutineSort::Title,
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list[0].routine.id, "apple");
    assert_eq!(list[1].routine.id, "banana");
    assert_eq!(list[2].routine.id, "cherry");
}

#[test]
fn svc_create_rejects_duplicate_slug() {
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
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                prompt: "p".into(),
                repositories: vec![],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        )
        .unwrap();

        let conflict = svc_create(
            &store,
            CreateRoutineRequest {
                schedule: "@daily".into(),
                // Different casing/spacing, same slug.
                title: "  svc create   DUP zzz ".into(),
                agent: "claude".into(),
                prompt: "p".into(),
                repositories: vec![],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        );
        assert!(matches!(conflict, Err(AppError::Conflict(_))));

        svc_delete(&store, &first.routine.id).unwrap();
    });
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_update_rejects_renaming_into_existing_slug() {
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
                schedule: None,
                // Rename "other" into the slug already owned by "keep".
                title: Some(title_keep.into()),
                agent: None,
                prompt: None,
                repositories: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        );
        assert!(matches!(conflict, Err(AppError::Conflict(_))));
    });

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title_keep));
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title_other));
}

#[test]
fn svc_update_sets_ttl_secs() {
    // Covers the `req.ttl_secs` apply branch in `svc_update`.
    let title = "Svc Update Ttl ZZZ";
    let store = new_store();
    let routine = make_routine("ttl-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("ttl-id".into(), routine);

    // `with_empty_path` keeps the post-update crontab sync from touching the real
    // crontab (issue #175): the update succeeds, the sync just warns.
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "ttl-id",
            UpdateRoutineRequest {
                schedule: None,
                title: None,
                agent: None,
                prompt: None,
                repositories: None,
                enabled: None,
                ttl_secs: Some(4242),
                max_runtime_secs: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.ttl_secs, Some(4242));
    });

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_update_sets_max_runtime_secs() {
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
                schedule: None,
                title: None,
                agent: None,
                prompt: None,
                repositories: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: Some(1234),
            },
        )
        .unwrap();
        assert_eq!(updated.routine.max_runtime_secs, Some(1234));
    });

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_logs_returns_newest_workbench_log() {
    // Covers the newest-workbench selection inside `svc_logs`: with two valid
    // `{slug}-{ts}` workbench directories, the higher timestamp wins and its
    // `agent.log` contents are returned.
    let title = "Svc Logs Newest ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let mut routine = make_routine("logs-id", title, 1, 1);
    routine.repositories = vec![Repository {
        repository: "https://example.com/r.git".into(),
        branch: None,
    }];
    store.lock().unwrap().insert("logs-id".into(), routine);

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    let older = workbenches.join(format!("{slug}-1000"));
    let newer = workbenches.join(format!("{slug}-2000"));
    std::fs::create_dir_all(&older).unwrap();
    std::fs::create_dir_all(&newer).unwrap();
    std::fs::write(older.join("agent.log"), "old log contents").unwrap();
    std::fs::write(newer.join("agent.log"), "new log contents").unwrap();

    let logs = svc_logs(&store, "logs-id").unwrap();
    assert_eq!(logs, "new log contents");

    let _ = std::fs::remove_dir_all(&older);
    let _ = std::fs::remove_dir_all(&newer);
}

#[test]
fn svc_logs_skips_foreign_and_unparseable_workbenches() {
    // Exercises the read_dir loop body across every arm: a workbench whose name
    // does not parse as `{slug}-{ts}` (parser returns None → skipped), a workbench
    // that parses but belongs to a different routine (`dir_slug != slug` → skipped),
    // and finally this routine's own workbench whose log is returned.
    let title = "Svc Logs Mixed ZZQ";
    let slug = slugify(title);
    let store = new_store();
    let routine = make_routine("logs-mixed-id", title, 1, 1);
    store
        .lock()
        .unwrap()
        .insert("logs-mixed-id".into(), routine);

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();

    // Not a `{slug}-{ts}` directory at all: parse_workbench_name returns None.
    let unparseable = workbenches.join("not-a-workbench-name");
    std::fs::create_dir_all(&unparseable).unwrap();
    std::fs::write(unparseable.join("agent.log"), "ignored").unwrap();

    // A well-formed workbench owned by a *different* routine slug.
    let foreign = workbenches.join("some-other-routine-9999");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("agent.log"), "foreign log").unwrap();

    // This routine's own workbench.
    let mine = workbenches.join(format!("{slug}-4242"));
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(mine.join("agent.log"), "mine log contents").unwrap();

    let logs = svc_logs(&store, "logs-mixed-id").unwrap();
    assert_eq!(logs, "mine log contents");

    let _ = std::fs::remove_dir_all(&unparseable);
    let _ = std::fs::remove_dir_all(&foreign);
    let _ = std::fs::remove_dir_all(&mine);
}

#[test]
fn svc_logs_empty_when_workbenches_dir_absent() {
    // Covers the `read_dir` error path in `svc_logs`: with `HOME` redirected to a fresh
    // temp dir, `workbenches_dir()` does not exist, so `std::fs::read_dir` returns Err and
    // the loop is skipped entirely. With no workbench found, the function returns an empty
    // string.
    let title = "Svc Logs No Workbenches ZZQ";
    let store = new_store();
    store.lock().unwrap().insert(
        "logs-empty-id".into(),
        make_routine("logs-empty-id", title, 1, 1),
    );

    let fresh_home = std::env::temp_dir().join(format!("moadim-no-wb-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&fresh_home).unwrap();
    let saved = std::env::var_os("HOME");
    // SAFETY: single-threaded test harness (RUST_TEST_THREADS=1); restored immediately after.
    unsafe {
        std::env::set_var("HOME", &fresh_home);
    }
    assert!(!crate::paths::workbenches_dir().exists());

    let logs = svc_logs(&store, "logs-empty-id").unwrap();

    unsafe {
        match saved {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }
    assert_eq!(logs, "");
    let _ = std::fs::remove_dir_all(&fresh_home);
}

#[test]
fn svc_logs_missing_routine_not_found() {
    assert!(matches!(
        svc_logs(&new_store(), "nope"),
        Err(AppError::NotFound)
    ));
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
        .unwrap_or_else(|poison| poison.into_inner());
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
fn svc_create_warns_when_crontab_sync_fails() {
    // With `PATH` cleared the `crontab` binary cannot be spawned, so
    // `sync_routines_to_crontab` errors and `svc_create` logs the warning but
    // still returns the created routine.
    let title = "Svc Create Sync Fail ZZZ";
    let store = new_store();
    with_empty_path(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                prompt: "p".into(),
                repositories: vec![],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        )
        .unwrap();
        assert_eq!(created.routine.title, title);
    });
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_update_warns_when_crontab_sync_fails() {
    // Same crontab-spawn failure as above, on the update path.
    let title = "Svc Update Sync Fail ZZZ";
    let store = new_store();
    let routine = make_routine("upd-sync-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-sync-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-sync-id",
            UpdateRoutineRequest {
                schedule: None,
                title: None,
                agent: None,
                prompt: Some("changed".into()),
                repositories: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.prompt, "changed");
    });
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_delete_warns_when_crontab_sync_fails() {
    // Same crontab-spawn failure, on the delete path.
    let title = "Svc Delete Sync Fail ZZZ";
    let store = new_store();
    let routine = make_routine("del-sync-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("del-sync-id".into(), routine);
    with_empty_path(|| {
        let deleted = svc_delete(&store, "del-sync-id").unwrap();
        assert_eq!(deleted.routine.title, title);
    });
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_trigger_warns_when_spawn_fails() {
    // With `PATH` cleared and an agent config present, `build_routine_command`
    // produces a command that `sh -c` cannot run because `sh` itself is not on
    // `PATH`, so the spawn fails and the warning branch runs. The trigger still
    // records its timestamp and returns.
    let agent_name = "svc-trigger-spawn-fail-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = []\n").unwrap();

    let title = "Svc Trigger Spawn Fail ZZZ";
    let store = new_store();
    let mut routine = make_routine("trig-spawn-id", title, 1, 1);
    routine.agent = agent_name.into();
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("trig-spawn-id".into(), routine);

    with_empty_path(|| {
        let triggered = svc_trigger(&store, "trig-spawn-id").unwrap();
        assert!(triggered.last_triggered_at.is_some());
    });

    let _ = std::fs::remove_file(&cfg);
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_create_rejects_unknown_agent() {
    // Covers the agent-validation branch in `svc_create`: an agent name that is
    // not in the registry must fail loud with `BadRequest` instead of being
    // persisted and silently skipped at fire time (#139).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: "Svc Create Unknown Agent ZZZ".into(),
            agent: "no-such-agent-zzz".into(),
            prompt: "p".into(),
            repositories: vec![],
            enabled: true,
            ttl_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // Nothing should have been persisted.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_accepts_builtin_agent() {
    // Covers the success path of agent validation: a built-in agent
    // (`ensure_default_agents` seeds `claude`/`codex`) is accepted.
    crate::routines::ensure_default_agents();
    let title = "Svc Create Valid Agent ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: title.into(),
            agent: "claude".into(),
            prompt: "p".into(),
            repositories: vec![],
            enabled: true,
            ttl_secs: None,
        },
    )
    .unwrap();
    assert_eq!(created.routine.agent, "claude");

    svc_delete(&store, &created.routine.id).unwrap();
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_update_rejects_unknown_agent() {
    // Covers the agent-validation branch in `svc_update`: updating a routine's
    // agent to an unknown name must fail with `BadRequest` before persisting.
    let title = "Svc Update Unknown Agent ZZZ";
    let store = new_store();
    let routine = make_routine("upd-agent-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-agent-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-agent-id",
        UpdateRoutineRequest {
            schedule: None,
            title: None,
            agent: Some("no-such-agent-zzz".into()),
            prompt: None,
            repositories: None,
            enabled: None,
            ttl_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // The stored routine keeps its original (valid) agent.
    assert_eq!(
        store.lock().unwrap().get("upd-agent-id").unwrap().agent,
        "claude"
    );

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

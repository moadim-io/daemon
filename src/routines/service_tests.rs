#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use crate::routines::{new_store, slugify, Repository};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing the
/// env var and the temp dir on drop. This keeps `svc_create`/`svc_update`/`write_routine` and the
/// other disk-touching paths off the developer's real `~/.moadim`, so a panicking assertion can never
/// leak test routines into the real home. Tests in this crate run single-threaded
/// (`RUST_TEST_THREADS=1`), so the global env mutation is safe.
struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> TempHome {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        TempHome(dir)
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
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
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
    let _home = TempHome::set();
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
    let _home = TempHome::set();
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

/// Build a minimal valid create request; callers tweak the field under test.
fn valid_create_request() -> CreateRoutineRequest {
    CreateRoutineRequest {
        schedule: "@daily".into(),
        title: "Valid Title".into(),
        agent: "claude".into(),
        prompt: "do the thing".into(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// Build a no-op update request (every field `None`); callers set one field.
fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        schedule: None,
        title: None,
        agent: None,
        prompt: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

#[test]
fn svc_create_rejects_blank_title() {
    let _home = TempHome::set();
    // Covers the `reject_blank("title", ..)` error arm in `svc_create`: a
    // whitespace-only title is refused before any slug/disk work (#226).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            title: "   ".into(),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_blank_prompt() {
    let _home = TempHome::set();
    // Covers the `reject_blank("prompt", ..)` error arm in `svc_create`: an empty
    // prompt would make the routine fire forever with no task (#224).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            prompt: String::new(),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_zero_ttl_secs() {
    let _home = TempHome::set();
    // Covers the `reject_zero_secs("ttl_secs", ..)` error arm in `svc_create`:
    // a zero TTL reaps finished-run logs instantly (#233).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            ttl_secs: Some(0),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_zero_max_runtime_secs() {
    let _home = TempHome::set();
    // Covers the `reject_zero_secs("max_runtime_secs", ..)` error arm in
    // `svc_create`: a zero cap self-kills the run immediately (#233).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            max_runtime_secs: Some(0),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_persists_machines() {
    let _home = TempHome::set();
    // Covers the `machines: req.machines` assignment in `svc_create`.
    let store = new_store();
    let resp = svc_create(
        &store,
        CreateRoutineRequest {
            machines: vec!["alpha".into(), "beta".into()],
            ..valid_create_request()
        },
    )
    .expect("create");
    assert_eq!(resp.routine.machines, vec!["alpha", "beta"]);
}

#[test]
fn svc_update_sets_machines() {
    let _home = TempHome::set();
    // Covers the `if let Some(machines) = req.machines` branch in `svc_update`.
    let store = store_with(vec![make_routine("upd-machines", "Keep", 1, 1)]);
    let resp = svc_update(
        &store,
        "upd-machines",
        UpdateRoutineRequest {
            machines: Some(vec!["server".into()]),
            ..empty_update_request()
        },
    )
    .expect("update");
    assert_eq!(resp.routine.machines, vec!["server"]);
}

#[test]
fn svc_update_rejects_blank_title() {
    let _home = TempHome::set();
    // Covers the `reject_blank("title", ..)` error arm in `svc_update`.
    let store = store_with(vec![make_routine("upd-blank-title", "Keep", 1, 1)]);
    let result = svc_update(
        &store,
        "upd-blank-title",
        UpdateRoutineRequest {
            title: Some("  ".into()),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_update_rejects_blank_prompt() {
    let _home = TempHome::set();
    // Covers the `reject_blank("prompt", ..)` error arm in `svc_update`.
    let store = store_with(vec![make_routine("upd-blank-prompt", "Keep", 1, 1)]);
    let result = svc_update(
        &store,
        "upd-blank-prompt",
        UpdateRoutineRequest {
            prompt: Some("\t\n".into()),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_update_rejects_zero_durations() {
    let _home = TempHome::set();
    // Covers both `reject_zero_secs` error arms on the update path.
    let store = store_with(vec![make_routine("upd-zero-secs", "Keep", 1, 1)]);
    let ttl = svc_update(
        &store,
        "upd-zero-secs",
        UpdateRoutineRequest {
            ttl_secs: Some(0),
            ..empty_update_request()
        },
    );
    assert!(matches!(ttl, Err(AppError::BadRequest(_))));
    let max_runtime = svc_update(
        &store,
        "upd-zero-secs",
        UpdateRoutineRequest {
            max_runtime_secs: Some(0),
            ..empty_update_request()
        },
    );
    assert!(matches!(max_runtime, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_ttl_above_cron_ceiling() {
    let _home = TempHome::set();
    // A `*/5 * * * *` routine has a ttl ceiling of min(3600, 300) = 300s. An explicit 1800 would be
    // silently clamped to 300, so it is rejected with `BadRequest` up front (#468).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "*/5 * * * *".into(),
            ttl_secs: Some(1800),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_max_runtime_above_cron_ceiling() {
    let _home = TempHome::set();
    // Mirror of the ttl ceiling for the watchdog bound (#468).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "*/5 * * * *".into(),
            max_runtime_secs: Some(1800),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_accepts_secs_at_cron_ceiling() {
    let _home = TempHome::set();
    // A value equal to the cron-derived ceiling (`*/5` -> 300s) is in force, not clamped, so it
    // passes `reject_over_ceiling` (covering the `secs <= ceiling` arm for both fields). A
    // duplicate-slug routine pre-seeded in the store makes the create fail *after* that check with a
    // `Conflict`, so the assertion proves the ceiling check did not reject — without performing any
    // crontab/disk mutation.
    let store = store_with(vec![make_routine(
        "at-ceiling-dupe",
        "At Ceiling ZZZ",
        1,
        1,
    )]);
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "*/5 * * * *".into(),
            // Same slug as the pre-seeded routine.
            title: "  at   ceiling ZZZ ".into(),
            ttl_secs: Some(300),
            max_runtime_secs: Some(300),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[test]
fn svc_update_rejects_ttl_above_current_schedule_ceiling() {
    let _home = TempHome::set();
    // No schedule supplied: the ceiling derives from the routine's *current* `*/5` schedule, so a
    // 1800s ttl exceeds the 300s ceiling and is rejected without mutating the store (#468).
    let store = store_with(vec![Routine {
        schedule: "*/5 * * * *".to_string(),
        ..make_routine("upd-ttl-ceiling", "Keep Ceiling", 1, 1)
    }]);
    let result = svc_update(
        &store,
        "upd-ttl-ceiling",
        UpdateRoutineRequest {
            ttl_secs: Some(1800),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // The store value is untouched by the rejected update.
    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("upd-ttl-ceiling")
            .unwrap()
            .ttl_secs,
        None
    );
}

#[test]
fn svc_update_rejects_secs_above_new_schedule_ceiling() {
    let _home = TempHome::set();
    // A supplied schedule is the *effective* schedule for the ceiling: tightening a `@daily` routine
    // to `*/5` while setting max_runtime 1800 exceeds the new 300s ceiling and is rejected (#468).
    let store = store_with(vec![make_routine("upd-new-sched", "Keep New Sched", 1, 1)]);
    let result = svc_update(
        &store,
        "upd-new-sched",
        UpdateRoutineRequest {
            schedule: Some("*/5 * * * *".into()),
            max_runtime_secs: Some(1800),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
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
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                prompt: "p".into(),
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
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
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        );
        assert!(matches!(conflict, Err(AppError::Conflict(_))));

        svc_delete(&store, &first.routine.id).unwrap();
    });
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
            schedule: "@daily".into(),
            title: "Svc Create Malformed ZZZ".into(),
            agent: agent_name.into(),
            prompt: "p".into(),
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
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
            schedule: "@daily".into(),
            title: "Svc Create Unreadable ZZZ".into(),
            agent: agent_name.into(),
            prompt: "p".into(),
            repositories: vec![],
            machines: vec![],
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
            schedule: None,
            title: None,
            agent: Some(agent_name.into()),
            prompt: None,
            repositories: None,
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
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
                schedule: None,
                // Rename "other" into the slug already owned by "keep".
                title: Some(title_keep.into()),
                agent: None,
                prompt: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        );
        assert!(matches!(conflict, Err(AppError::Conflict(_))));
    });
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
                schedule: None,
                title: None,
                agent: None,
                prompt: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: Some(1800),
                max_runtime_secs: None,
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
                schedule: None,
                title: None,
                agent: None,
                prompt: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: Some(1234),
            },
        )
        .unwrap();
        assert_eq!(updated.routine.max_runtime_secs, Some(1234));
    });
}

#[test]
fn svc_logs_returns_newest_workbench_log() {
    let _home = TempHome::set();
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
}

#[test]
fn svc_logs_skips_foreign_and_unparseable_workbenches() {
    let _home = TempHome::set();
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
}

#[test]
fn svc_logs_empty_when_workbenches_dir_absent() {
    let _home = TempHome::set();
    // Covers the `read_dir` error path in `svc_logs`: the fresh temp home has no `workbenches`
    // subdirectory, so `std::fs::read_dir` returns Err and the loop is skipped entirely. With no
    // workbench found, the function returns an empty string.
    let title = "Svc Logs No Workbenches ZZQ";
    let store = new_store();
    store.lock().unwrap().insert(
        "logs-empty-id".into(),
        make_routine("logs-empty-id", title, 1, 1),
    );

    assert!(!crate::paths::workbenches_dir().exists());

    let logs = svc_logs(&store, "logs-empty-id").unwrap();
    assert_eq!(logs, "");
}

#[test]
fn svc_logs_missing_routine_not_found() {
    let _home = TempHome::set();
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
fn svc_create_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
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
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        )
        .unwrap();
        assert_eq!(created.routine.title, title);
    });
}

#[test]
fn svc_update_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
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
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.prompt, "changed");
    });
}

#[test]
fn svc_delete_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
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
}

/// Run `body` with `MOADIM_CRONTAB_BIN` pointed at a shim that succeeds (`crontab -l` prints an
/// empty crontab and exits 0; `crontab -` swallows stdin and exits 0), so the crontab sync returns
/// `Ok` and the non-error branch of `svc_create`/`svc_update`/`svc_delete` is exercised without
/// touching the developer's real crontab. The prior env value is restored and the temp dir removed.
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
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                prompt: "p".into(),
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
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
                schedule: None,
                title: None,
                agent: None,
                prompt: Some("changed".into()),
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
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

#[test]
fn svc_trigger_warns_when_spawn_fails() {
    let _home = TempHome::set();
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
        assert!(triggered.last_manual_trigger_at.is_some());
    });
}

#[test]
fn svc_trigger_scheduled_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_trigger_scheduled(&new_store(), "nope"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_trigger_scheduled_spawns_without_recording_manual_trigger() {
    let _home = TempHome::set();
    // The scheduled path must leave `last_manual_trigger_at` untouched (it is for *manual* triggers
    // only); `with_empty_path` makes the spawn fail so the test never launches a real session, while
    // still exercising the spawn branch.
    let agent_name = "svc-trigger-scheduled-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = []\n").unwrap();

    let title = "Svc Trigger Scheduled ZZZ";
    let store = new_store();
    let mut routine = make_routine("trig-sched-id", title, 1, 1);
    routine.agent = agent_name.into();
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("trig-sched-id".into(), routine);

    with_empty_path(|| {
        let triggered = svc_trigger_scheduled(&store, "trig-sched-id").unwrap();
        assert!(triggered.last_manual_trigger_at.is_none());
    });
}

/// Build a create request with the given title and an otherwise-valid body.
fn create_req_with_title(title: &str) -> CreateRoutineRequest {
    CreateRoutineRequest {
        schedule: "@daily".into(),
        title: title.into(),
        agent: "claude".into(),
        prompt: "p".into(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

#[test]
fn svc_create_rejects_blank_and_punctuation_titles() {
    let _home = TempHome::set();
    // Covers `validate_title`'s alphanumeric-required reject branch via `svc_create`:
    // empty, whitespace-only, and punctuation-only titles all 400 before any
    // persistence or crontab sync, leaving the store empty (issue #226).
    for title in ["", "   \n\t", "!!!"] {
        let store = new_store();
        let result = svc_create(&store, create_req_with_title(title));
        assert!(
            matches!(result, Err(AppError::BadRequest(_))),
            "title {title:?} should be rejected"
        );
        assert!(store.lock().unwrap().is_empty());
    }
}

#[test]
fn svc_create_rejects_overlong_title() {
    let _home = TempHome::set();
    // Covers `validate_title`'s max-length reject branch: a title past
    // `MAX_TITLE_LEN` characters 400s even though it has alphanumerics.
    let store = new_store();
    let title = "a".repeat(MAX_TITLE_LEN + 1);
    let result = svc_create(&store, create_req_with_title(&title));
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_unknown_agent() {
    let _home = TempHome::set();
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
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // Nothing should have been persisted.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_update_rejects_blank_and_punctuation_titles() {
    let _home = TempHome::set();
    // Covers the `req.title` validation branch in `svc_update`: renaming an
    // existing routine to an empty, whitespace-only, or punctuation-only title
    // 400s and leaves the stored title untouched (issue #226).
    let original = "Svc Update Title Guard ZZZ";
    for title in ["", "   ", "!!!"] {
        let store = new_store();
        let routine = make_routine("title-guard-id", original, 1, 1);
        crate::routine_storage::write_routine(&routine).unwrap();
        store
            .lock()
            .unwrap()
            .insert("title-guard-id".into(), routine);

        let result = svc_update(
            &store,
            "title-guard-id",
            UpdateRoutineRequest {
                schedule: None,
                title: Some(title.into()),
                agent: None,
                prompt: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        );
        assert!(
            matches!(result, Err(AppError::BadRequest(_))),
            "update to title {title:?} should be rejected"
        );
        assert_eq!(
            store.lock().unwrap().get("title-guard-id").unwrap().title,
            original
        );
    }
}

#[test]
fn svc_create_accepts_builtin_agent() {
    let _home = TempHome::set();
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
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    )
    .unwrap();
    assert_eq!(created.routine.agent, "claude");

    svc_delete(&store, &created.routine.id).unwrap();
}

#[test]
fn svc_update_rejects_unknown_agent() {
    let _home = TempHome::set();
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
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // The stored routine keeps its original (valid) agent.
    assert_eq!(
        store.lock().unwrap().get("upd-agent-id").unwrap().agent,
        "claude"
    );
}

#[test]
fn svc_create_rejects_blank_repository_url() {
    let _home = TempHome::set();
    // Covers the repositories-validation branch in `svc_create` (#241): an entry
    // whose URL is empty or whitespace-only must fail loud with `BadRequest`
    // instead of being stored and rendered as a broken `- ` clone bullet.
    let store = new_store();
    for url in ["", "   "] {
        let result = svc_create(
            &store,
            CreateRoutineRequest {
                schedule: "@daily".into(),
                title: "Svc Create Blank Repo ZZZ".into(),
                agent: "claude".into(),
                prompt: "p".into(),
                repositories: vec![Repository {
                    repository: url.into(),
                    branch: None,
                }],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
            },
        );
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }
    // Nothing should have been persisted.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_blank_repository_branch() {
    let _home = TempHome::set();
    // Covers the optional-branch guard: a `Some` branch that is empty/whitespace
    // must be rejected so `compose_prompt` cannot emit `- url (branch )`.
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: "Svc Create Blank Branch ZZZ".into(),
            agent: "claude".into(),
            prompt: "p".into(),
            repositories: vec![Repository {
                repository: "https://github.com/octocat/Hello-World".into(),
                branch: Some("  ".into()),
            }],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_trims_repository_entries() {
    let _home = TempHome::set();
    // Covers the normalization path: surrounding whitespace on a valid URL/branch
    // is trimmed before storing, so the rendered preamble bullet is clean.
    crate::routines::ensure_default_agents();
    let title = "Svc Create Trim Repo ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: title.into(),
            agent: "claude".into(),
            prompt: "p".into(),
            repositories: vec![Repository {
                repository: "  https://github.com/octocat/Hello-World  ".into(),
                branch: Some("  main  ".into()),
            }],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    )
    .unwrap();
    let repo = &created.routine.repositories[0];
    assert_eq!(repo.repository, "https://github.com/octocat/Hello-World");
    assert_eq!(repo.branch.as_deref(), Some("main"));

    svc_delete(&store, &created.routine.id).unwrap();
}

#[test]
fn svc_update_rejects_blank_repository_url() {
    let _home = TempHome::set();
    // Covers the repositories-validation branch in `svc_update`: replacing the
    // list with a blank-URL entry must fail with `BadRequest` before persisting.
    let title = "Svc Update Blank Repo ZZZ";
    let store = new_store();
    let routine = make_routine("upd-repo-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-repo-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-repo-id",
        UpdateRoutineRequest {
            schedule: None,
            title: None,
            agent: None,
            prompt: None,
            repositories: Some(vec![Repository {
                repository: " ".into(),
                branch: None,
            }]),
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // The stored routine keeps its original (empty) repository list.
    assert!(store
        .lock()
        .unwrap()
        .get("upd-repo-id")
        .unwrap()
        .repositories
        .is_empty());
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

#[test]
fn svc_create_rejects_empty_prompt() {
    // Covers `validate_prompt`'s reject branch via `svc_create`: an empty prompt
    // is a 400 before any persistence or crontab sync (issue #224).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: "Svc Create Empty Prompt ZZZ".into(),
            agent: "claude".into(),
            prompt: "".into(),
            repositories: vec![],
            machines: vec![],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // No routine was created, so the store stays empty.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_whitespace_prompt() {
    // A whitespace-only prompt trims to empty and is rejected like a blank one.
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: "Svc Create Whitespace Prompt ZZZ".into(),
            agent: "claude".into(),
            prompt: "   \n\t".into(),
            repositories: vec![],
            machines: vec![],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_update_rejects_clearing_prompt_to_empty() {
    // Covers the `req.prompt` validation branch in `svc_update`: updating an
    // existing routine's prompt to whitespace-only is a 400, and the stored
    // prompt is left untouched (issue #224).
    let title = "Svc Update Empty Prompt ZZZ";
    let store = new_store();
    let routine = make_routine("empty-prompt-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("empty-prompt-id".into(), routine);

    let result = svc_update(
        &store,
        "empty-prompt-id",
        UpdateRoutineRequest {
            schedule: None,
            title: None,
            agent: None,
            prompt: Some("   ".into()),
            repositories: None,
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert_eq!(
        store.lock().unwrap().get("empty-prompt-id").unwrap().prompt,
        "do the thing"
    );

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

// ─── New tests for previously uncovered lines ────────────────────────────────

#[test]
fn svc_list_local_only_filters_non_matching_machines() {
    let _home = TempHome::set();
    // Covers L137: the `local_only=true` retain path. A routine whose `machines` list
    // does not contain the current machine is dropped; one that does is kept.
    let local = make_routine("list-local-id", "List Local Machine ZZZ", 1, 1);
    // `make_routine` seeds `machines: [current_machine()]`, so this one passes the filter.
    let mut other = make_routine("list-other-id", "List Other Machine ZZZ", 2, 2);
    other.machines = vec!["definitely-not-this-machine-xyz".to_string()];
    let store = store_with(vec![local, other]);
    let query = RoutineListQuery {
        local_only: Some(true),
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].routine.id, "list-local-id");
}

#[cfg(unix)]
#[test]
fn svc_create_returns_internal_on_write_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L304: `write_routine(..).map_err(|_| AppError::Internal)?` in `svc_create`.
    // The slug dir is pre-created with a `.gitignore` (so that step is skipped),
    // then made read-only so the atomic write of `routine.toml` fails.
    let _home = TempHome::set();
    let title = "Svc Create Write Fail ZZZ";
    let slug = slugify(title);
    let dir = crate::paths::routine_dir(&slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        crate::paths::routine_gitignore_path(&slug),
        "*.local.*\n*.log\nrun.sh\n",
    )
    .unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            title: title.into(),
            ..valid_create_request()
        },
    );

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
    // Nothing should have been inserted into the store.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_update_none_schedule_uses_existing_schedule() {
    let _home = TempHome::set();
    // Covers L359: the `None => lock.get(id)?.schedule.clone()` arm. When no new
    // schedule is supplied the ceiling check must derive from the stored schedule.
    let store = new_store();
    let routine = make_routine("upd-none-sched-id", "Upd None Sched ZZZ", 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("upd-none-sched-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-none-sched-id",
            UpdateRoutineRequest {
                prompt: Some("updated prompt".into()),
                ..empty_update_request()
            },
        )
        .expect("update should succeed");
        assert_eq!(updated.routine.schedule, "@daily");
    });
}

#[test]
fn svc_update_with_explicit_schedule_applies_it() {
    let _home = TempHome::set();
    // Covers L371: `lock.get_mut(id).ok_or(AppError::NotFound)?`. When `req.schedule`
    // is `Some`, the `Some` arm at L358 is taken, and the code reaches L371 to mutate
    // the routine in place.
    let store = new_store();
    let routine = make_routine("upd-expl-sched-id", "Upd Explicit Sched ZZZ", 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("upd-expl-sched-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-expl-sched-id",
            UpdateRoutineRequest {
                schedule: Some("@daily".into()),
                ..empty_update_request()
            },
        )
        .expect("update should succeed");
        assert_eq!(updated.routine.schedule, "@daily");
    });
}

#[cfg(unix)]
#[test]
fn svc_update_returns_internal_on_write_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L403: `write_routine(..).map_err(|_| AppError::Internal)?` in `svc_update`.
    // The slug dir is made read-only after the routine is written to disk, so the
    // re-persist inside `svc_update` cannot create a new temp file.
    let _home = TempHome::set();
    let title = "Svc Update Write Fail ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let routine = make_routine("upd-write-fail-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("upd-write-fail-id".into(), routine);

    let dir = crate::paths::routine_dir(&slug);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_update(
        &store,
        "upd-write-fail-id",
        UpdateRoutineRequest {
            prompt: Some("changed".into()),
            ..empty_update_request()
        },
    );

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[cfg(unix)]
#[test]
fn svc_update_returns_internal_on_remove_dir_failure_after_title_change() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L405: `remove_routine_dir(&old_slug).map_err(|_| AppError::Internal)?`.
    // write_routine for the NEW slug succeeds (its dir is pre-created and writable);
    // removing the OLD slug dir fails because the parent `routines/` is read-only.
    let _home = TempHome::set();
    let old_title = "Svc Update Old Remove ZZZ";
    let new_title = "Svc Update New Remove ZZZ";
    let new_slug = slugify(new_title);

    let store = new_store();
    let routine = make_routine("upd-rm-fail-id", old_title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("upd-rm-fail-id".into(), routine);

    // Pre-create the new slug dir so write_routine can succeed without creating it.
    let new_dir = crate::paths::routine_dir(&new_slug);
    std::fs::create_dir_all(&new_dir).unwrap();

    // Make the routines/ parent read-only: write inside existing subdirs still works
    // (directory permission is on the parent, not subdirs), but removing an entry from
    // it (old slug dir) fails.
    let routines = crate::paths::routines_dir();
    std::fs::set_permissions(&routines, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_update(
        &store,
        "upd-rm-fail-id",
        UpdateRoutineRequest {
            title: Some(new_title.into()),
            ..empty_update_request()
        },
    );

    std::fs::set_permissions(&routines, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[cfg(unix)]
#[test]
fn svc_delete_returns_internal_on_remove_dir_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L416: `remove_routine_dir(..).map_err(|_| AppError::Internal)?` in `svc_delete`.
    // The routine is removed from the in-memory store, but removing its on-disk dir fails
    // because the parent `routines/` dir is read-only.
    let _home = TempHome::set();
    let title = "Svc Delete Remove Fail ZZZ";
    let store = new_store();
    let routine = make_routine("del-rm-fail-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("del-rm-fail-id".into(), routine);

    let routines = crate::paths::routines_dir();
    std::fs::set_permissions(&routines, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_delete(&store, "del-rm-fail-id");

    std::fs::set_permissions(&routines, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
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
fn svc_update_not_found_when_no_schedule_and_id_missing() {
    let _home = TempHome::set();
    // Covers L359 error arm: `lock.get(id).ok_or(AppError::NotFound)?` fires when the
    // routine does not exist in the store and no new schedule is provided.
    let store = new_store(); // empty store
    with_empty_path(|| {
        let result = svc_update(
            &store,
            "nonexistent-id",
            UpdateRoutineRequest {
                schedule: None,
                ..empty_update_request()
            },
        );
        assert!(matches!(result, Err(AppError::NotFound)));
    });
}

#[test]
fn svc_update_not_found_when_schedule_provided_and_id_missing() {
    let _home = TempHome::set();
    // Covers L371 error arm: `lock.get_mut(id).ok_or(AppError::NotFound)?` fires when
    // the routine does not exist in the store and a new schedule is provided.
    let store = new_store(); // empty store
    with_empty_path(|| {
        let result = svc_update(
            &store,
            "nonexistent-id",
            UpdateRoutineRequest {
                schedule: Some("@daily".into()),
                ..empty_update_request()
            },
        );
        assert!(matches!(result, Err(AppError::NotFound)));
    });
}

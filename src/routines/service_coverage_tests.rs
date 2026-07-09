#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::{new_store, slugify};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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

/// Wrap a list of routines into a populated [`RoutineStore`], and persist each one to disk (under
/// the active `TempHome`) so `svc_list`'s reload-from-disk doesn't wipe the fixture the test just
/// built — disk is the source of truth the reload re-scans on every call.
fn store_with(routines: Vec<Routine>) -> RoutineStore {
    let mut map = HashMap::new();
    for routine in routines {
        write_routine(&routine).expect("write_routine");
        map.insert(routine.id.clone(), routine);
    }
    Arc::new(Mutex::new(map))
}

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

static PATH_GUARD: Mutex<()> = Mutex::new(());

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
    let list = svc_list(&store, &crate::paths::routines_dir(), &query);
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
            model: None,
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
fn svc_create_rejects_blank_tag() {
    // Covers the tags-validation error branch in `svc_create`: a blank or
    // whitespace-only tag must 400 before anything is persisted. `ensure_default_agents`
    // makes the agent check pass so validation reaches `validate_tags`.
    crate::routines::ensure_default_agents();
    let store = new_store();
    for tag in ["", "   "] {
        let result = svc_create(
            &store,
            CreateRoutineRequest {
                model: None,
                tags: vec![tag.to_string()],
                ..valid_create_request()
            },
        );
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }
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
                model: None,
                prompt: Some("updated prompt".into()),
                goal: None,
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
                model: None,
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
            model: None,
            prompt: Some("changed".into()),
            goal: None,
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
            model: None,
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

#[test]
fn svc_delete_kills_the_routines_in_flight_workbench_session() {
    // #333: deleting a routine while its agent is mid-run must not leave that run executing
    // unsupervised until the next TTL sweep. Covers the `killed > 0` log::warn! branch in
    // `svc_delete`, backed by a tmux stub that reports every session as alive.
    let _home = TempHome::set();
    let prev_tmux = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: single-threaded test execution; restored below.
    unsafe {
        std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/true");
    }

    let title = "Svc Delete Kills Session ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let routine = make_routine("del-kill-id", title, 1, 1);
    store.lock().unwrap().insert("del-kill-id".into(), routine);

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(workbenches.join(format!("{slug}-1"))).unwrap();

    let result = svc_delete(&store, "del-kill-id");

    // SAFETY: single-threaded test execution.
    unsafe {
        match prev_tmux {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }

    assert!(result.is_ok());
}

#[test]
fn svc_update_not_found_when_id_missing() {
    let _home = TempHome::set();
    // `svc_update` looks the id up once (to compute `old_slug`) while holding the store's
    // lock for the rest of the function, so a missing id can only ever fail at that single,
    // first lookup — regardless of whether a new schedule is supplied. This one test covers
    // both request shapes; the later `lock.get`/`lock.get_mut` calls can no longer fail on a
    // missing id, so they use `.expect(..)` instead of a second/third `NotFound` arm.
    let store = new_store(); // empty store
    with_empty_path(|| {
        for schedule in [None, Some("@daily".to_string())] {
            let result = svc_update(
                &store,
                "nonexistent-id",
                UpdateRoutineRequest {
                    schedule,
                    ..empty_update_request()
                },
            );
            assert!(matches!(result, Err(AppError::NotFound)));
        }
    });
}

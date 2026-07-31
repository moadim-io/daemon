#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
                schedules: None,
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

#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[cfg(unix)]
#[test]
fn svc_trigger_scheduled_returns_internal_on_write_failure_when_decrementing_skip_runs() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L603: `write_routine(..).map_err(|_| AppError::Internal)?` in the
    // skip_runs-decrement arm of `svc_trigger_scheduled`.
    let _home = TempHome::set();
    let title = "Sched Skip Runs Write Fail ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let mut routine = make_routine("sched-skip-write-fail-id", title, 1, 1);
    routine.skip_runs = Some(2);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("sched-skip-write-fail-id".into(), routine);

    let dir = crate::paths::routine_dir(&slug);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_trigger_scheduled(&store, "sched-skip-write-fail-id");

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[test]
fn svc_trigger_scheduled_skip_runs_clears_at_zero_then_spawns_next_fire() {
    let _home = TempHome::set();
    let agent_name = "svc-sched-skip-zero-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    std::fs::write(
        crate::paths::agent_toml_path(agent_name),
        "command = \"true\"\nargs = []\n",
    )
    .unwrap();

    let store = new_store();
    let mut routine = make_routine("sched-skip-zero-id", "Sched Skip Zero ZZZ", 1, 1);
    routine.agent = agent_name.into();
    routine.skip_runs = Some(1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("sched-skip-zero-id".into(), routine);

    // First fire: the last skip, skip_runs clears to None.
    let first = svc_trigger_scheduled(&store, "sched-skip-zero-id");
    assert!(matches!(first, Err(AppError::Locked(_))));
    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("sched-skip-zero-id")
            .unwrap()
            .skip_runs,
        None
    );

    // Second fire: nothing left to skip, spawns normally.
    with_empty_path(|| {
        let second = svc_trigger_scheduled(&store, "sched-skip-zero-id").unwrap();
        assert_eq!(second.skip_runs, None);
    });
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

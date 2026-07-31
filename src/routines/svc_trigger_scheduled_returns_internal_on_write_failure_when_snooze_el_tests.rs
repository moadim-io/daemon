
#[cfg(unix)]
#[test]
fn svc_trigger_scheduled_returns_internal_on_write_failure_when_snooze_elapses() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L594: `write_routine(..).map_err(|_| AppError::Internal)?` in the
    // snoozed-until-elapsed arm of `svc_trigger_scheduled`.
    let _home = TempHome::set();
    let title = "Sched Snooze Write Fail ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let mut routine = make_routine("sched-snooze-write-fail-id", title, 1, 1);
    routine.snoozed_until = Some(1); // long past
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("sched-snooze-write-fail-id".into(), routine);

    let dir = crate::paths::routine_dir(&slug);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_trigger_scheduled(&store, "sched-snooze-write-fail-id");

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[test]
fn svc_trigger_scheduled_skip_runs_zero_spawns_normally() {
    // skip_runs: Some(0) is a degenerate but reachable state (e.g. svc_snooze called with
    // skip_runs: Some(0)) and must behave like None: nothing to skip, spawn as normal.
    let _home = TempHome::set();
    let agent_name = "svc-sched-skip-runs-zero-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    std::fs::write(
        crate::paths::agent_toml_path(agent_name),
        "command = \"true\"\nargs = []\n",
    )
    .unwrap();

    let store = new_store();
    let mut routine = make_routine("sched-skip-runs-zero-id", "Sched Skip Runs Zero ZZZ", 1, 1);
    routine.agent = agent_name.into();
    routine.skip_runs = Some(0);
    store
        .lock()
        .unwrap()
        .insert("sched-skip-runs-zero-id".into(), routine);

    with_empty_path(|| {
        let triggered = svc_trigger_scheduled(&store, "sched-skip-runs-zero-id").unwrap();
        assert_eq!(triggered.skip_runs, Some(0));
    });
}

#[test]
fn svc_trigger_scheduled_decrements_skip_runs_without_spawning() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("sched-skip-runs-id", "Sched Skip Runs ZZZ", 1, 1);
    routine.skip_runs = Some(2);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("sched-skip-runs-id".into(), routine);

    let result = svc_trigger_scheduled(&store, "sched-skip-runs-id");
    assert!(
        matches!(result, Err(AppError::Locked(_))),
        "expected Locked error, got {result:?}"
    );
    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("sched-skip-runs-id")
            .unwrap()
            .skip_runs,
        Some(1),
        "skip_runs must decrement in the in-memory store, not just on disk"
    );
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_trigger_scheduled_returns_internal_on_write_failure_when_decrement_tests.rs"]
mod svc_trigger_scheduled_returns_internal_on_write_failure_when_decrement_tests;

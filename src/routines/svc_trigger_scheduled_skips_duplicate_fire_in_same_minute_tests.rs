#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_trigger_scheduled_skips_duplicate_fire_in_same_minute() {
    while now_secs() % 60 > 54 {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("trig-sched-dedupe-id", "Trig Sched Dedupe ZZZ", 1, 1);
    routine.last_scheduled_trigger_at = Some(now_secs());
    store
        .lock()
        .unwrap()
        .insert("trig-sched-dedupe-id".into(), routine);

    let result = svc_trigger_scheduled(&store, "trig-sched-dedupe-id");

    assert!(
        matches!(result, Err(AppError::Locked(ref msg)) if msg.contains("already fired this minute")),
        "expected same-minute scheduled fire dedupe, got {result:?}"
    );
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
        let scheduled_log = crate::paths::routine_scheduled_log_path(&slugify(title));
        let timestamp = std::fs::read_to_string(&scheduled_log)
            .expect("accepted scheduled fire is durable even when launch fails");
        assert!(
            timestamp.trim().parse::<u64>().is_ok(),
            "expected a Unix timestamp in {}: {timestamp:?}",
            scheduled_log.display()
        );
    });
}

#[test]
fn svc_trigger_manual_bypasses_snooze() {
    let _home = TempHome::set();
    let agent_name = "svc-trigger-bypass-snooze-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    std::fs::write(
        crate::paths::agent_toml_path(agent_name),
        "command = \"true\"\nargs = []\n",
    )
    .unwrap();

    let store = new_store();
    let mut routine = make_routine("trig-bypass-snooze-id", "Trig Bypass Snooze ZZZ", 1, 1);
    routine.agent = agent_name.into();
    routine.snoozed_until = Some(now_secs() + 3600);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("trig-bypass-snooze-id".into(), routine);

    with_empty_path(|| {
        let triggered = svc_trigger(&store, "trig-bypass-snooze-id").unwrap();
        assert!(triggered.last_manual_trigger_at.is_some());
        // Manual trigger ignores snooze entirely: the field is left untouched.
        assert!(triggered.snoozed_until.is_some());
    });
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
include!("svc_create_syncs_crontab_on_success_tests.rs");

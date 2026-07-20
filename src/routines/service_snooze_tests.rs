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
        env: std::collections::HashMap::new(),
    }
}

/// Serializes the tests that clear `PATH`, so concurrent service tests never see
/// a stripped environment. The poisoned-lock case is recovered into the guard.
static PATH_GUARD: Mutex<()> = Mutex::new(());

/// Run `body` with an empty `PATH`, restoring the original value afterwards.
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
fn svc_trigger_scheduled_skips_when_snoozed_until_future() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("sched-snooze-future-id", "Sched Snooze Future ZZZ", 1, 1);
    routine.snoozed_until = Some(now_secs() + 3600);
    store
        .lock()
        .unwrap()
        .insert("sched-snooze-future-id".into(), routine);

    let result = svc_trigger_scheduled(&store, "sched-snooze-future-id");
    assert!(
        matches!(result, Err(AppError::Locked(_))),
        "expected Locked error, got {result:?}"
    );
    // No workbench spawn attempted and no disk write: snoozed_until survives unchanged in-store.
    assert!(store
        .lock()
        .unwrap()
        .get("sched-snooze-future-id")
        .unwrap()
        .snoozed_until
        .is_some());
}

#[test]
fn svc_trigger_scheduled_clears_snoozed_until_once_elapsed_and_spawns() {
    let _home = TempHome::set();
    let agent_name = "svc-sched-snooze-elapsed-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    std::fs::write(
        crate::paths::agent_toml_path(agent_name),
        "command = \"true\"\nargs = []\n",
    )
    .unwrap();

    let store = new_store();
    let mut routine = make_routine("sched-snooze-elapsed-id", "Sched Snooze Elapsed ZZZ", 1, 1);
    routine.agent = agent_name.into();
    routine.snoozed_until = Some(1); // long past
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("sched-snooze-elapsed-id".into(), routine);

    with_empty_path(|| {
        let triggered = svc_trigger_scheduled(&store, "sched-snooze-elapsed-id").unwrap();
        assert_eq!(triggered.snoozed_until, None);
    });
    // The in-memory store reflects the clear too, not just the returned value.
    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("sched-snooze-elapsed-id")
            .unwrap()
            .snoozed_until,
        None
    );
}

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

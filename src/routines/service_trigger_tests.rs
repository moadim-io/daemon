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
fn svc_trigger_skips_spawn_when_prompt_exceeds_inline_limit() {
    let _home = TempHome::set();
    // An agent whose args inline `{prompt}`, combined with a composed prompt over the
    // inline-argument limit, must skip the spawn (#443) rather than launch a command doomed to
    // fail silently inside tmux with `E2BIG`. The trigger still records its timestamp and
    // returns Ok — the same non-fatal shape as `svc_trigger_warns_when_spawn_fails` above. `PATH`
    // is left as-is (unlike that test): the skip must happen before a spawn is ever attempted, not
    // because the shell can't be found.
    let agent_name = "svc-trigger-oversized-prompt-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = [\"{prompt}\"]\n").unwrap();

    let title = "Svc Trigger Oversized Prompt ZZZ";
    let store = new_store();
    let mut routine = make_routine("trig-oversized-id", title, 1, 1);
    routine.agent = agent_name.into();
    routine.prompt = "x".repeat(crate::routines::MAX_INLINE_PROMPT_BYTES * 2);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("trig-oversized-id".into(), routine);

    let triggered = svc_trigger(&store, "trig-oversized-id").unwrap();
    assert!(triggered.last_manual_trigger_at.is_some());
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

#[test]
fn strip_ansi_noise_leaves_plain_text_untouched() {
    assert_eq!(
        strip_ansi_noise("plain log line\nsecond line\n"),
        "plain log line\nsecond line\n"
    );
}

#[test]
fn strip_ansi_noise_removes_csi_color_codes() {
    assert_eq!(strip_ansi_noise("\u{1B}[31mred\u{1B}[0m\n"), "red\n");
}

#[test]
fn strip_ansi_noise_removes_osc_sequence_terminated_by_bel() {
    assert_eq!(
        strip_ansi_noise("\u{1B}]0;window title\u{7}after\n"),
        "after\n"
    );
}

#[test]
fn strip_ansi_noise_removes_osc_sequence_terminated_by_escape_backslash() {
    assert_eq!(
        strip_ansi_noise("\u{1B}]0;window title\u{1B}\\after\n"),
        "after\n"
    );
}

#[test]
fn strip_ansi_noise_drops_bare_two_byte_escape() {
    // `ESC c` is a full terminal reset with no CSI/OSC bracket.
    assert_eq!(strip_ansi_noise("before\u{1B}cafter\n"), "beforeafter\n");
}

#[test]
fn strip_ansi_noise_drops_trailing_lone_escape() {
    assert_eq!(strip_ansi_noise("before\u{1B}"), "before");
}

#[test]
fn strip_ansi_noise_collapses_carriage_return_redraws() {
    assert_eq!(
        strip_ansi_noise("progress: 10%\rprogress: 100%\ndone\n"),
        "progress: 100%\ndone\n"
    );
}

#[test]
fn strip_ansi_noise_handles_combined_escape_and_redraw_noise() {
    assert_eq!(
        strip_ansi_noise("\u{1B}[2K\u{1B}[1Gspin .\rspin ..\rspin ...\ndone\u{1B}[0m\n"),
        "spin ...\ndone\n"
    );
}

#[test]
fn read_log_tail_strips_ansi_noise_from_a_whole_file_read() {
    let dir = std::env::temp_dir().join(format!("moadim-logtail-ansi-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agent.log");
    std::fs::write(&path, "\u{1B}[31mred\u{1B}[0m line\rreal line\n").unwrap();

    assert_eq!(read_log_tail(&path).unwrap(), "real line\n");
    let _ = std::fs::remove_dir_all(&dir);
}

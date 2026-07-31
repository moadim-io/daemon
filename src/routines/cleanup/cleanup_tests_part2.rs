#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn reap_dir_kills_hung_session_over_max_runtime_then_reaps() {
    let base = std::env::temp_dir().join("moadim-cleanup-watchdog-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "hung-100"); // live + over max runtime -> killed, then reaped

    let now = 1000;
    let ttl_for = |_slug: &str| 500_u64; // age 900 > 500 -> TTL elapsed
    let max_runtime_for = |_slug: &str| 300_u64; // age 900 > 300 -> watchdog trips
    let alive = |_session: &str| true; // session is still running
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let stats = reap_dir(
        &base,
        now,
        &ttl_for,
        &max_runtime_for,
        &alive,
        &kill,
        &finish_at_trigger,
        &noop_persist,
    );

    assert_eq!(stats.removed, 1, "hung-then-killed workbench is reaped");
    assert_eq!(killed.into_inner(), vec!["moadim-hung-100".to_string()]);
    assert!(!base.join("hung-100").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn reap_dir_records_forced_kill_in_agent_log_when_ttl_not_yet_elapsed() {
    let base = std::env::temp_dir().join("moadim-cleanup-watchdog-log-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "hung-900"); // live + over max runtime, but TTL not yet elapsed

    let now = 1000;
    let ttl_for = |_slug: &str| 100_000_u64; // age 100 <= huge TTL -> not reaped this sweep
    let max_runtime_for = |_slug: &str| 50_u64; // age 100 > 50 -> watchdog trips
    let alive = |_session: &str| true;
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let stats = reap_dir(
        &base,
        now,
        &ttl_for,
        &max_runtime_for,
        &alive,
        &kill,
        &finish_at_trigger,
        &noop_persist,
    );

    assert_eq!(
        stats.removed, 0,
        "killed but TTL not elapsed -> left for a later sweep"
    );
    assert_eq!(killed.into_inner(), vec!["moadim-hung-900".to_string()]);
    // The forced termination is recorded in the run's agent.log.
    let log = std::fs::read_to_string(base.join("hung-900").join("agent.log")).unwrap();
    assert!(log.contains("exceeded max runtime"));
    // ...and the run's exit_code records the distinct `killed` sentinel (not a misleading `0`),
    // so a watchdog-killed run is distinguishable from a clean exit (#453).
    let exit_code = std::fs::read_to_string(base.join("hung-900").join("exit_code")).unwrap();
    assert_eq!(exit_code.trim(), "killed");

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn reap_dir_does_not_kill_dead_session_missing_tmux() {
    // Mirrors the missing-tmux fallback: is_alive reports false (no tmux / session gone), so the
    // watchdog never kills, and an expired finished run is reaped normally.
    let base = std::env::temp_dir().join("moadim-cleanup-watchdog-dead-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "gone-100"); // over both bounds but already dead

    let now = 1000;
    let ttl_for = |_slug: &str| 100_u64;
    let max_runtime_for = |_slug: &str| 100_u64;
    let dead = |_session: &str| false;
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let stats = reap_dir(
        &base,
        now,
        &ttl_for,
        &max_runtime_for,
        &dead,
        &kill,
        &finish_at_trigger,
        &noop_persist,
    );

    assert_eq!(stats.removed, 1);
    assert!(
        killed.into_inner().is_empty(),
        "no kill for an already-dead session"
    );

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn kill_sessions_for_slug_kills_only_live_matching_sessions() {
    // #333: deleting a routine must not leave its in-flight workbench session running.
    let base = std::env::temp_dir().join("moadim-cleanup-kill-slug-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    touch_dir(&base, "deleted-100"); // matching slug, live      -> killed
    touch_dir(&base, "deleted-200"); // matching slug, dead      -> left alone (already gone)
    touch_dir(&base, "other-100"); // different slug, live       -> untouched
    touch_dir(&base, "notawb"); // no timestamp, ignored
    std::fs::write(base.join("deleted-stray"), b"x").unwrap(); // a file, not a dir -> ignored

    let alive = |session: &str| session == "moadim-deleted-100" || session == "moadim-other-100";
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let count = kill_sessions_for_slug(&base, "deleted", &alive, &kill);

    assert_eq!(count, 1);
    assert_eq!(killed.into_inner(), vec!["moadim-deleted-100".to_string()]);

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn kill_sessions_for_slug_returns_zero_for_a_missing_dir() {
    let missing =
        std::env::temp_dir().join(format!("moadim-kill-slug-missing-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&missing);
    let dead = |_session: &str| false;
    assert_eq!(
        kill_sessions_for_slug(&missing, "anything", &dead, &|_| {}),
        0
    );
}

#[test]
fn kill_sessions_for_deleted_routine_kills_the_live_workbench_session() {
    let home = std::env::temp_dir().join(format!(
        "moadim-cleanup-kill-deleted-{}",
        uuid::Uuid::new_v4()
    ));
    let prev_home = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_tmux = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
        std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/true");
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    std::fs::create_dir_all(workbenches.join("deleted-routine-1")).unwrap();
    std::fs::create_dir_all(workbenches.join("other-routine-1")).unwrap();

    let killed = kill_sessions_for_deleted_routine("deleted-routine");
    assert_eq!(
        killed, 1,
        "only the deleted routine's live session is killed"
    );
    // The workbench directory itself is left in place; only the session is force-killed.
    assert!(workbenches.join("deleted-routine-1").exists());
    assert!(workbenches.join("other-routine-1").exists());

    // SAFETY: single-threaded harness; restore the saved overrides.
    unsafe {
        match prev_home {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
        match prev_tmux {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

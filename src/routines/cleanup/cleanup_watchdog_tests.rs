#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]
use super::runtime::MAX_RUNTIME_SECS;
use super::ttl::MAX_TTL_SECS;
use super::*;
use crate::utils::time::now_secs;

/// A `max_runtime_for` that never trips the watchdog (so reap tests exercise TTL only).
fn never_expires_runtime(_slug: &str) -> u64 {
    u64::MAX
}

/// A `kill` that does nothing (the watchdog is not expected to fire).
fn noop_kill(_session: &str) {}

/// A `finished_at` that reports each run's trigger timestamp as its finish time, isolating the TTL
/// math from `agent.log` mtime so a test asserts reap decisions purely on the injected `ts`.
fn finish_at_trigger(_dir: &std::path::Path, trigger_ts: u64) -> u64 {
    trigger_ts
}

/// A `persist` that does nothing — these reap tests aren't exercising durable history.
fn noop_persist(
    _slug: &str,
    _name: &str,
    _path: &std::path::Path,
    _started_at: u64,
    _finished_at: u64,
) {
}

fn touch_dir(parent: &std::path::Path, name: &str) {
    std::fs::create_dir_all(parent.join(name)).unwrap();
}

#[test]
fn watchdog_dir_kills_hung_session_without_reaping() {
    let base = std::env::temp_dir().join("moadim-watchdog-kill-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "hung-100"); // live + over max runtime -> killed
    touch_dir(&base, "fresh-900"); // live + within bound      -> untouched
    touch_dir(&base, "gone-100"); // already dead              -> untouched
    touch_dir(&base, "notawb"); // not a workbench            -> skipped
    std::fs::write(base.join("stray-50"), b"x").unwrap(); // a file, not a dir -> ignored

    let now = 1000;
    let max_runtime_for = |_slug: &str| 300_u64; // age 900 (hung/gone) > 300, age 100 (fresh) <= 300
    let alive = |session: &str| session != "moadim-gone-100";
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let count = watchdog_dir(&base, now, &max_runtime_for, &alive, &kill);

    assert_eq!(count, 1, "only the hung session is killed");
    assert_eq!(killed.into_inner(), vec!["moadim-hung-100".to_string()]);
    // The watchdog only kills; it never reaps, so every directory still exists.
    assert!(base.join("hung-100").exists());
    assert!(base.join("fresh-900").exists());
    assert!(base.join("gone-100").exists());
    // The kill is recorded in the run's agent.log.
    let log = std::fs::read_to_string(base.join("hung-100").join("agent.log")).unwrap();
    assert!(log.contains("exceeded max runtime"));

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn watchdog_dir_returns_zero_when_dir_unreadable() {
    let missing =
        std::env::temp_dir().join(format!("moadim-watchdog-missing-{}", uuid::Uuid::new_v4()));
    assert!(!missing.exists());
    let max_runtime_for = |_slug: &str| 0_u64;
    let alive = |_session: &str| true;
    assert_eq!(
        watchdog_dir(&missing, 1000, &max_runtime_for, &alive, &noop_kill),
        0
    );
}

#[test]
fn kill_hung_sessions_scans_real_workbenches_dir() {
    // Drives the public watchdog entry point so it resolves the real `workbenches_dir()` and runs
    // `tmux_session_alive` as the injected liveness check. With an empty store every slug falls back
    // to MAX_RUNTIME_SECS, so the temp workbench (no live tmux session) is never killed — but the
    // snapshot + scan path executes.
    let home = std::env::temp_dir().join(format!("moadim-watchdog-{}", uuid::Uuid::new_v4()));
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    // An old workbench whose tmux session is absent -> not killed (already dead).
    std::fs::create_dir_all(workbenches.join("orphan-1")).unwrap();

    let store = super::super::model::new_store();
    let killed = kill_hung_sessions(&store);

    assert_eq!(killed, 0);
    assert!(workbenches.join("orphan-1").exists());

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn reap_dir_returns_zero_when_dir_unreadable() {
    // A directory that does not exist makes `read_dir` fail; the early `return 0`
    // branch is taken and nothing is reaped.
    let missing =
        std::env::temp_dir().join(format!("moadim-cleanup-missing-{}", uuid::Uuid::new_v4()));
    assert!(!missing.exists());
    let ttl_for = |_slug: &str| 0_u64;
    let dead = |_session: &str| false;
    assert_eq!(
        reap_dir(
            &missing,
            1000,
            &ttl_for,
            &never_expires_runtime,
            &dead,
            &noop_kill,
            &finish_at_trigger,
            &noop_persist
        ),
        ReapStats::default()
    );
}
include!("cleanup_watchdog_tests_part3.rs");

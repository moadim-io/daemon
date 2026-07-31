#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn cleanup_expired_workbenches_scans_real_workbenches_dir() {
    // Drives the public entry point so `cleanup_expired_workbenches` resolves the real
    // `workbenches_dir()` (honouring MOADIM_HOME_OVERRIDE) and `tmux_session_alive` runs as the
    // injected liveness check. With an empty store every slug falls back to MAX_TTL_SECS, so we
    // stamp the expired workbench far enough in the past to exceed that cap.
    let home = std::env::temp_dir().join(format!("moadim-cleanup-{}", uuid::Uuid::new_v4()));
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    // An expired (timestamp 1) finished workbench whose tmux session is absent -> reaped.
    std::fs::create_dir_all(workbenches.join("orphan-1")).unwrap();
    // A workbench triggered "now-ish" so it is not yet expired -> kept.
    let fresh_ts = now_secs();
    std::fs::create_dir_all(workbenches.join(format!("recent-{fresh_ts}"))).unwrap();
    // A non-workbench directory (no numeric suffix) -> skipped.
    std::fs::create_dir_all(workbenches.join("notawb")).unwrap();

    let store = crate::routines::new_store();
    let stats = cleanup_expired_workbenches(&store);

    // The orphaned, expired, session-less workbench is removed; the others survive.
    assert!(
        stats.removed >= 1,
        "expected at least the orphan to be reaped"
    );
    assert!(!workbenches.join("orphan-1").exists());
    assert!(workbenches.join(format!("recent-{fresh_ts}")).exists());
    assert!(workbenches.join("notawb").exists());

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[cfg(unix)]
#[test]
fn cleanup_expired_workbenches_kills_a_live_hung_session() {
    use std::os::unix::fs::PermissionsExt as _;

    // Drives the public entry point against a *live* session so the watchdog path runs end-to-end:
    // a stub `tmux` that always exits 0 makes `tmux_session_alive` report the session as running
    // (exercising its `status.success()` mapping over a real process), which in turn makes
    // `cleanup_expired_workbenches` consult its `max_runtime_for` bound. An ancient timestamp puts
    // the run past the (empty-store default) max runtime, so the session is force-killed, the kill
    // is noted in agent.log, and the workbench is reaped. Complements
    // `cleanup_expired_workbenches_scans_real_workbenches_dir`, which covers the no-tmux path.
    let home = std::env::temp_dir().join(format!("moadim-cleanup-hung-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    // A stub tmux that ignores its args and always succeeds, so has-session/kill-session both "work".
    let stub_tmux = home.join("stub-tmux");
    std::fs::write(&stub_tmux, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&stub_tmux, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev_home = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_tmux = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
        std::env::set_var("MOADIM_TMUX_BIN", &stub_tmux);
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    // Timestamp 1 → far past any max-runtime / TTL bound, and its session reports alive via the stub.
    std::fs::create_dir_all(workbenches.join("hung-1")).unwrap();

    let store = crate::routines::new_store();
    let stats = cleanup_expired_workbenches(&store);

    assert_eq!(
        stats.removed, 1,
        "the live-but-overrun workbench is killed then reaped"
    );
    assert!(!workbenches.join("hung-1").exists());

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

#[test]
fn effective_ttl_caps_at_max_for_long_intervals() {
    // Daily interval (24h) is well above the 1h cap, so retention is the cap.
    assert_eq!(
        routine_with("@daily", None).effective_ttl_secs(),
        MAX_TTL_SECS
    );
}

#[test]
fn effective_ttl_follows_sub_hour_cron_interval() {
    // Every 10 minutes -> retention is the 600s interval, below the cap.
    assert_eq!(
        routine_with("*/10 * * * *", None).effective_ttl_secs(),
        10 * 60
    );
}

#[test]
fn effective_ttl_explicit_only_lowers() {
    // An explicit ttl_secs below the cap wins.
    assert_eq!(routine_with("@daily", Some(42)).effective_ttl_secs(), 42);
    // An explicit ttl_secs above the cap is clamped down to it.
    assert_eq!(
        routine_with("@daily", Some(u64::MAX)).effective_ttl_secs(),
        MAX_TTL_SECS
    );
    // It cannot raise retention above the smaller cron interval either.
    assert_eq!(
        routine_with("*/10 * * * *", Some(u64::MAX)).effective_ttl_secs(),
        10 * 60
    );
}

#[test]
fn effective_ttl_falls_back_to_cap_for_unparseable_schedule() {
    assert_eq!(
        routine_with("@reboot", None).effective_ttl_secs(),
        MAX_TTL_SECS
    );
}

#[test]
fn effective_max_runtime_defaults_to_cap_when_unset() {
    // Daily interval (24h) is above the 1h cap, so the bound is the cap.
    assert_eq!(
        routine_with("@daily", None).effective_max_runtime_secs(),
        MAX_RUNTIME_SECS
    );
}

#[test]
fn effective_max_runtime_follows_sub_hour_cron_interval() {
    // Every 10 minutes -> the bound is the 600s interval, below the cap.
    let mut routine = routine_with("*/10 * * * *", None);
    assert_eq!(routine.effective_max_runtime_secs(), 10 * 60);
    // An explicit value can only lower it further, never raise it above the cron-derived cap.
    routine.max_runtime_secs = Some(u64::MAX);
    assert_eq!(routine.effective_max_runtime_secs(), 10 * 60);
}
include!("effective_max_runtime_uses_explicit_value_tests.rs");

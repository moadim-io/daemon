#![allow(clippy::missing_docs_in_private_items)]

use super::*;

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

#[test]
fn tmux_kill_session_is_best_effort_on_missing_session() {
    // The real tmux side-effect helper. Killing a session that does not exist (or running with no
    // tmux installed) must not panic — the call is best-effort and any failure is swallowed.
    tmux_kill_session("moadim-nonexistent-watchdog-test-session");
}

#[test]
fn tmux_bin_falls_back_to_a_nonexistent_path_under_cfg_test() {
    // Without an override, the test-build fallback must NOT be the real `tmux` binary, and must
    // point at a path that does not exist, so probes/kills are harmless no-ops (#215). This mirrors
    // the crontab_bin guard (#211).
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var("MOADIM_TMUX_BIN");
    }

    let bin = super::session::tmux_bin();
    assert_ne!(bin, "tmux", "test build must not spawn the real tmux");
    assert!(
        !std::path::Path::new(&bin).exists(),
        "test fallback must point at a non-existent path: {bin}"
    );

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
}

#[test]
fn tmux_session_alive_reflects_the_bin_exit_status() {
    // Point the tmux seam at real binaries that exit 0 / non-zero so the `has-session` status
    // actually resolves, exercising the `.map(|status| status.success())` branch in both directions
    // (the cfg(test) fallback path never spawns a real process, so this is the only place it runs).
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    let set = |bin: &str| {
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe { std::env::set_var("MOADIM_TMUX_BIN", bin) };
    };

    set("/usr/bin/true");
    assert!(
        super::session::tmux_session_alive("moadim-anything"),
        "a 0-exit tmux stub reads as alive"
    );
    set("/usr/bin/false");
    assert!(
        !super::session::tmux_session_alive("moadim-anything"),
        "a non-zero-exit tmux stub reads as not alive"
    );

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
}

#[test]
fn note_forced_kill_is_silent_when_log_cannot_be_opened() {
    // The workbench directory does not exist, so opening `agent.log` (append+create) fails because
    // its parent is absent — exercising the `if let Ok` fall-through (the best-effort Err branch).
    let missing = std::env::temp_dir().join(format!("moadim-nfk-missing-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&missing);
    super::session::note_forced_kill(&missing);
    // Nothing is created when the open fails.
    assert!(!missing.exists());
}

#[test]
fn cleanup_expired_workbenches_kills_a_live_expired_session() {
    // With the tmux seam pointed at a 0-exit stub every session reads as *alive*, so the watchdog's
    // `alive && is_expired(.., max_runtime_for(slug))` actually evaluates the `max_runtime_for`
    // closure (the existing test's absent-tmux path short-circuits before it). The expired workbench
    // is force-killed and reaped.
    let home = std::env::temp_dir().join(format!("moadim-cleanup-live-{}", uuid::Uuid::new_v4()));
    let prev_home = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_tmux = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
        std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/true");
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    // Timestamp 1 → far past any default max-runtime ceiling → expired, so the live session is killed.
    std::fs::create_dir_all(workbenches.join("alive-1")).unwrap();

    let store = super::super::model::new_store();
    let removed = cleanup_expired_workbenches(&store);
    assert!(
        removed >= 1,
        "the live, expired workbench is killed and reaped"
    );
    assert!(!workbenches.join("alive-1").exists());

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
fn tmux_bin_honors_the_override_env_var() {
    // With `MOADIM_TMUX_BIN` set, `tmux_bin` returns it verbatim, ahead of the cfg(test) fallback —
    // the seam that lets a test point probes/kills at a controlled stub.
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_TMUX_BIN", "/tmp/moadim-test-tmux-override");
    }

    assert_eq!(super::session::tmux_bin(), "/tmp/moadim-test-tmux-override");

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
}

#[test]
fn parse_workbench_name_splits_slug_and_timestamp() {
    assert_eq!(parse_workbench_name("foo-123"), Some(("foo", 123)));
    // Slug may contain dashes; only the final all-digit segment is the timestamp.
    assert_eq!(
        parse_workbench_name("my-routine-1700000000"),
        Some(("my-routine", 1_700_000_000))
    );
}

#[test]
fn parse_workbench_name_rejects_non_workbench_dirs() {
    assert_eq!(parse_workbench_name("noseparator"), None);
    assert_eq!(parse_workbench_name("foo-bar"), None); // suffix not numeric
    assert_eq!(parse_workbench_name("foo-"), None); // empty timestamp
    assert_eq!(parse_workbench_name("-123"), None); // empty slug
}

#[test]
fn is_expired_compares_age_against_ttl() {
    assert!(is_expired(1000, 0, 500)); // age 1000 > ttl 500
    assert!(!is_expired(1000, 600, 500)); // age 400 <= ttl 500
    assert!(!is_expired(1000, 1000, 0)); // age 0, never expired at ttl 0
                                         // Trigger timestamp in the future (clock skew) saturates to age 0.
    assert!(!is_expired(1000, 2000, 0));
}

fn touch_dir(parent: &std::path::Path, name: &str) {
    std::fs::create_dir_all(parent.join(name)).unwrap();
}

#[test]
fn reap_dir_removes_only_finished_and_expired() {
    let base = std::env::temp_dir().join("moadim-cleanup-reap-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    touch_dir(&base, "expired-100"); // old + dead  -> removed
    touch_dir(&base, "fresh-900"); // recent       -> kept
    touch_dir(&base, "running-100"); // old but live -> kept
    touch_dir(&base, "notawb"); // no timestamp      -> kept
    std::fs::write(base.join("stray-50"), b"x").unwrap(); // a file, not a dir -> ignored

    let now = 1000;
    let ttl_for = |_slug: &str| 500u64; // expiry threshold: age > 500
    let alive = |session: &str| session == "moadim-running-100";

    let removed = reap_dir(
        &base,
        now,
        &ttl_for,
        &never_expires_runtime,
        &alive,
        &noop_kill,
        &finish_at_trigger,
    );

    assert_eq!(removed, 1);
    assert!(!base.join("expired-100").exists());
    assert!(base.join("fresh-900").exists());
    assert!(base.join("running-100").exists());
    assert!(base.join("notawb").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn reap_dir_uses_per_slug_ttl() {
    let base = std::env::temp_dir().join("moadim-cleanup-perslug-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    // Same age (500s) for both, but "short" has a tighter TTL so only it expires.
    touch_dir(&base, "short-500");
    touch_dir(&base, "long-500");

    let now = 1000;
    let ttl_for = |slug: &str| if slug == "short" { 100 } else { 100_000 };
    let dead = |_session: &str| false;

    let removed = reap_dir(
        &base,
        now,
        &ttl_for,
        &never_expires_runtime,
        &dead,
        &noop_kill,
        &finish_at_trigger,
    );

    assert_eq!(removed, 1);
    assert!(!base.join("short-500").exists());
    assert!(base.join("long-500").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn reap_dir_measures_ttl_from_finish_not_trigger() {
    // #174: retention is measured from when the run *finished*, not when it was triggered. A run
    // whose duration exceeds its TTL must still be kept for the full window after it completes,
    // while a long-finished run is reaped. Both dirs share trigger ts 100 (trigger-based age 900),
    // so a trigger-based reaper would delete both; finish-based keeps the just-finished one.
    let base = std::env::temp_dir().join("moadim-cleanup-finish-ttl-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    touch_dir(&base, "longrun-100"); // triggered at 100, finished recently (at 900)
    touch_dir(&base, "donelong-100"); // triggered at 100, finished long ago (at 100)

    let now = 1000;
    let ttl_for = |_slug: &str| 500u64; // retention window: 500s from finish
    let dead = |_session: &str| false;
    // Finish time is per-workbench: the long-running one finished at 900 (age 100 <= 500 -> kept);
    // the other finished at 100 (age 900 > 500 -> reaped). Run duration never eats the window.
    let finished_at = |dir: &std::path::Path, _ts: u64| {
        if dir.file_name().unwrap() == "longrun-100" {
            900
        } else {
            100
        }
    };

    let removed = reap_dir(
        &base,
        now,
        &ttl_for,
        &never_expires_runtime,
        &dead,
        &noop_kill,
        &finished_at,
    );

    assert_eq!(removed, 1, "only the long-finished run is reaped");
    assert!(
        base.join("longrun-100").exists(),
        "a run that finished within its TTL is retained even though its trigger age exceeds the TTL"
    );
    assert!(!base.join("donelong-100").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn agent_log_finish_time_falls_back_to_trigger_without_log() {
    // No agent.log present -> the trigger timestamp is used as the finish time.
    let base =
        std::env::temp_dir().join(format!("moadim-cleanup-finishfn-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    assert_eq!(agent_log_finish_time(&base, 4242), 4242);
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn agent_log_finish_time_uses_log_mtime_clamped_to_trigger() {
    // With an agent.log present, its mtime (a recent, large unix time) is used and is never less
    // than the trigger timestamp.
    let base = std::env::temp_dir().join(format!(
        "moadim-cleanup-finishfn-mtime-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("agent.log"), b"done\n").unwrap();
    // Trigger far in the past: the just-written log's mtime dominates, so finish > trigger.
    let finish = agent_log_finish_time(&base, 1);
    assert!(
        finish > 1,
        "fresh agent.log mtime should yield a finish time later than an ancient trigger"
    );
    // Trigger far in the future (clock skew): clamped up to the trigger, never below it.
    assert_eq!(agent_log_finish_time(&base, u64::MAX), u64::MAX);
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn reap_dir_kills_hung_session_over_max_runtime_then_reaps() {
    let base = std::env::temp_dir().join("moadim-cleanup-watchdog-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "hung-100"); // live + over max runtime -> killed, then reaped

    let now = 1000;
    let ttl_for = |_slug: &str| 500u64; // age 900 > 500 -> TTL elapsed
    let max_runtime_for = |_slug: &str| 300u64; // age 900 > 300 -> watchdog trips
    let alive = |_session: &str| true; // session is still running
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let removed = reap_dir(
        &base,
        now,
        &ttl_for,
        &max_runtime_for,
        &alive,
        &kill,
        &finish_at_trigger,
    );

    assert_eq!(removed, 1, "hung-then-killed workbench is reaped");
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
    let ttl_for = |_slug: &str| 100_000u64; // age 100 <= huge TTL -> not reaped this sweep
    let max_runtime_for = |_slug: &str| 50u64; // age 100 > 50 -> watchdog trips
    let alive = |_session: &str| true;
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let removed = reap_dir(
        &base,
        now,
        &ttl_for,
        &max_runtime_for,
        &alive,
        &kill,
        &finish_at_trigger,
    );

    assert_eq!(
        removed, 0,
        "killed but TTL not elapsed -> left for a later sweep"
    );
    assert_eq!(killed.into_inner(), vec!["moadim-hung-900".to_string()]);
    // The forced termination is recorded in the run's agent.log.
    let log = std::fs::read_to_string(base.join("hung-900").join("agent.log")).unwrap();
    assert!(log.contains("exceeded max runtime"));

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
    let ttl_for = |_slug: &str| 100u64;
    let max_runtime_for = |_slug: &str| 100u64;
    let dead = |_session: &str| false;
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let removed = reap_dir(
        &base,
        now,
        &ttl_for,
        &max_runtime_for,
        &dead,
        &kill,
        &finish_at_trigger,
    );

    assert_eq!(removed, 1);
    assert!(
        killed.into_inner().is_empty(),
        "no kill for an already-dead session"
    );

    std::fs::remove_dir_all(&base).unwrap();
}

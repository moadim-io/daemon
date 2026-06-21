#![allow(clippy::missing_docs_in_private_items)]

use super::runtime::MAX_RUNTIME_SECS;
use super::ttl::MAX_TTL_SECS;
use super::*;

/// A `max_runtime_for` that never trips the watchdog (so reap tests exercise TTL only).
fn never_expires_runtime(_slug: &str) -> u64 {
    u64::MAX
}

/// A `kill` that does nothing (the watchdog is not expected to fire).
fn noop_kill(_session: &str) {}

#[test]
fn tmux_kill_session_is_best_effort_on_missing_session() {
    // The real tmux side-effect helper. Killing a session that does not exist (or running with no
    // tmux installed) must not panic — the call is best-effort and any failure is swallowed.
    tmux_kill_session("moadim-nonexistent-watchdog-test-session");
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
    );

    assert_eq!(removed, 1);
    assert!(!base.join("short-500").exists());
    assert!(base.join("long-500").exists());

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

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &alive, &kill);

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

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &alive, &kill);

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

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &dead, &kill);

    assert_eq!(removed, 1);
    assert!(
        killed.into_inner().is_empty(),
        "no kill for an already-dead session"
    );

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn reap_dir_returns_zero_when_dir_unreadable() {
    // A directory that does not exist makes `read_dir` fail; the early `return 0`
    // branch is taken and nothing is reaped.
    let missing =
        std::env::temp_dir().join(format!("moadim-cleanup-missing-{}", uuid::Uuid::new_v4()));
    assert!(!missing.exists());
    let ttl_for = |_slug: &str| 0u64;
    let dead = |_session: &str| false;
    assert_eq!(
        reap_dir(
            &missing,
            1000,
            &ttl_for,
            &never_expires_runtime,
            &dead,
            &noop_kill
        ),
        0
    );
}

#[cfg(unix)]
#[test]
fn reap_dir_counts_zero_when_remove_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    // An expired + dead workbench whose removal fails (parent dir is read-only) is
    // not counted, exercising the `Err` arm of the remove match.
    let base = std::env::temp_dir().join(format!(
        "moadim-cleanup-removefail-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "expired-100");
    std::fs::write(base.join("expired-100").join("inner"), b"x").unwrap();

    // Strip write permission from the parent so removing the child directory fails.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&base, perms).unwrap();

    let now = 1000;
    let ttl_for = |_slug: &str| 500u64; // age 900 > 500 -> expired
    let dead = |_session: &str| false;
    let removed = reap_dir(
        &base,
        now,
        &ttl_for,
        &never_expires_runtime,
        &dead,
        &noop_kill,
    );
    // A read-only parent makes `remove_dir_all` fail for an unprivileged user, so
    // the directory survives and the Err arm runs (0 removed). Root bypasses the
    // permission check; tolerate that by only asserting consistency.
    if base.join("expired-100").exists() {
        assert_eq!(removed, 0);
    }

    // Restore permissions so cleanup can proceed.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&base, perms).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

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

    let store = super::super::model::new_store();
    let removed = cleanup_expired_workbenches(&store);

    // The orphaned, expired, session-less workbench is removed; the others survive.
    assert!(removed >= 1, "expected at least the orphan to be reaped");
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

fn routine_with(schedule: &str, ttl_secs: Option<u64>) -> super::super::model::Routine {
    super::super::model::Routine {
        id: "x".into(),
        schedule: schedule.into(),
        title: "t".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        repositories: vec![],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        tags: vec![],
        ttl_secs,
        max_runtime_secs: None,
    }
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

#[test]
fn effective_max_runtime_uses_explicit_value() {
    let mut routine = routine_with("@daily", None);
    routine.max_runtime_secs = Some(1234);
    assert_eq!(routine.effective_max_runtime_secs(), 1234);
    // An explicit value above the cap is clamped down to it.
    routine.max_runtime_secs = Some(u64::MAX);
    assert_eq!(routine.effective_max_runtime_secs(), MAX_RUNTIME_SECS);
}

#[test]
fn effective_ttl_falls_back_to_cap_when_schedule_never_fires() {
    // "Feb 30" parses as a valid cron expression but matches no real date, so the
    // schedule yields no future fire times. `cron_interval_secs` returns None at the
    // first `fires.next()?`, and `effective_ttl_secs` falls back to the cap.
    assert_eq!(
        routine_with("0 0 30 2 *", None).effective_ttl_secs(),
        MAX_TTL_SECS
    );
    // An explicit ttl below the cap still wins even when the interval can't be computed.
    assert_eq!(
        routine_with("0 0 30 2 *", Some(15)).effective_ttl_secs(),
        15
    );
}

#![allow(clippy::missing_docs_in_private_items)]

use super::ttl::MAX_TTL_SECS;
use super::*;

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
    let max_runtime_for = |_slug: &str| 100_000u64; // no live session is over max runtime here
    let alive = |session: &str| session == "moadim-running-100";
    let never_kill =
        |_session: &str| panic!("watchdog must not kill a session within its max runtime");

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &alive, &never_kill);

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
    let max_runtime_for = |_slug: &str| 100_000u64;
    let dead = |_session: &str| false;
    let never_kill = |_session: &str| panic!("no live session to kill");

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &dead, &never_kill);

    assert_eq!(removed, 1);
    assert!(!base.join("short-500").exists());
    assert!(base.join("long-500").exists());

    std::fs::remove_dir_all(&base).unwrap();
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
        last_triggered_at: None,
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
fn effective_max_runtime_uses_default_when_unset() {
    use super::ttl::DEFAULT_MAX_RUNTIME_SECS;
    let routine = routine_with("@daily", None);
    assert_eq!(
        routine.effective_max_runtime_secs(),
        DEFAULT_MAX_RUNTIME_SECS
    );
}

#[test]
fn effective_max_runtime_honors_explicit_value() {
    let mut routine = routine_with("@daily", None);
    routine.max_runtime_secs = Some(120);
    assert_eq!(routine.effective_max_runtime_secs(), 120);
}

#[test]
fn watchdog_kills_overrunning_session_then_reaps() {
    let base = std::env::temp_dir().join("moadim-cleanup-watchdog-kill-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    // A live run triggered at ts=100; at now=1000 it has run 900s.
    touch_dir(&base, "hung-100");

    let now = 1000;
    let ttl_for = |_slug: &str| 500u64; // age 900 > 500 -> reapable once finished
    let max_runtime_for = |_slug: &str| 300u64; // 900 > 300 -> over max runtime
    let alive = |session: &str| session == "moadim-hung-100";
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &alive, &kill);

    // Killed exactly the hung session...
    assert_eq!(killed.borrow().as_slice(), ["moadim-hung-100".to_string()]);
    // ...and, now finished, the workbench was reaped.
    assert_eq!(removed, 1);
    assert!(!base.join("hung-100").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn watchdog_records_forced_termination_in_agent_log() {
    let base = std::env::temp_dir().join("moadim-cleanup-watchdog-log-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "hung-100");

    let now = 1000;
    // TTL not yet elapsed, so the workbench survives the sweep and we can read its agent.log.
    let ttl_for = |_slug: &str| 100_000u64;
    let max_runtime_for = |_slug: &str| 300u64;
    let alive = |session: &str| session == "moadim-hung-100";
    let kill = |_session: &str| {};

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &alive, &kill);

    assert_eq!(removed, 0); // killed but not yet expired
    let log = std::fs::read_to_string(base.join("hung-100").join("agent.log")).unwrap();
    assert!(log.contains("exceeded max runtime"), "log was: {log:?}");
    assert!(log.contains("900s")); // runtime
    assert!(log.contains("300s")); // the limit it crossed

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn watchdog_keeps_session_within_max_runtime() {
    let base = std::env::temp_dir().join("moadim-cleanup-watchdog-keep-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "busy-100");

    let now = 1000; // run age 900
    let ttl_for = |_slug: &str| 0u64; // would be expired if it were finished
    let max_runtime_for = |_slug: &str| 100_000u64; // 900 < limit -> still within budget
    let alive = |session: &str| session == "moadim-busy-100";
    let never_kill = |_session: &str| panic!("a session within its max runtime must not be killed");

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &alive, &never_kill);

    assert_eq!(removed, 0);
    assert!(base.join("busy-100").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn watchdog_missing_tmux_falls_back_to_ttl_reaping() {
    // When tmux is missing, `is_alive` returns false: the run is treated as finished, so the
    // watchdog never kills and the workbench is reaped purely on TTL.
    let base = std::env::temp_dir().join("moadim-cleanup-watchdog-notmux-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "over-100");

    let now = 1000;
    let ttl_for = |_slug: &str| 500u64; // age 900 > 500 -> expired
    let max_runtime_for = |_slug: &str| 300u64; // over max runtime, but session reads as dead
    let dead = |_session: &str| false; // simulate missing tmux
    let never_kill = |_session: &str| panic!("nothing to kill when no session is alive");

    let removed = reap_dir(&base, now, &ttl_for, &max_runtime_for, &dead, &never_kill);

    assert_eq!(removed, 1);
    assert!(!base.join("over-100").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

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
    let alive = |session: &str| session == "moadim-running-100";

    let removed = reap_dir(&base, now, &ttl_for, &alive);

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

    let removed = reap_dir(&base, now, &ttl_for, &dead);

    assert_eq!(removed, 1);
    assert!(!base.join("short-500").exists());
    assert!(base.join("long-500").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn reap_dir_returns_zero_when_dir_unreadable() {
    // A directory that does not exist makes `read_dir` fail; the early `return 0`
    // branch is taken and nothing is reaped.
    let missing = std::env::temp_dir().join(format!("moadim-cleanup-missing-{}", uuid::Uuid::new_v4()));
    assert!(!missing.exists());
    let ttl_for = |_slug: &str| 0u64;
    let dead = |_session: &str| false;
    assert_eq!(reap_dir(&missing, 1000, &ttl_for, &dead), 0);
}

#[cfg(unix)]
#[test]
fn reap_dir_counts_zero_when_remove_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    // An expired + dead workbench whose removal fails (parent dir is read-only) is
    // not counted, exercising the `Err` arm of the remove match.
    let base = std::env::temp_dir().join(format!("moadim-cleanup-removefail-{}", uuid::Uuid::new_v4()));
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
    let removed = reap_dir(&base, now, &ttl_for, &dead);
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
fn effective_ttl_falls_back_to_cap_when_schedule_never_fires() {
    // "Feb 30" parses as a valid cron expression but matches no real date, so the
    // schedule yields no future fire times. `cron_interval_secs` returns None at the
    // first `fires.next()?`, and `effective_ttl_secs` falls back to the cap.
    assert_eq!(
        routine_with("0 0 30 2 *", None).effective_ttl_secs(),
        MAX_TTL_SECS
    );
    // An explicit ttl below the cap still wins even when the interval can't be computed.
    assert_eq!(routine_with("0 0 30 2 *", Some(15)).effective_ttl_secs(), 15);
}

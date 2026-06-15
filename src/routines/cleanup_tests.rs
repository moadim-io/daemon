#![allow(clippy::missing_docs_in_private_items)]

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
fn effective_ttl_falls_back_to_default() {
    let mut r = super::super::model::Routine {
        id: "x".into(),
        schedule: "@daily".into(),
        title: "t".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        repositories: vec![],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
        ttl_secs: None,
    };
    assert_eq!(r.effective_ttl_secs(), DEFAULT_TTL_SECS);
    r.ttl_secs = Some(42);
    assert_eq!(r.effective_ttl_secs(), 42);
}

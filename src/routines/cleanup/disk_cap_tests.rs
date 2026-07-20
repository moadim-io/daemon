#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn candidate(name: &str, size: u64, finish_ts: u64) -> EvictCandidate {
    EvictCandidate {
        name: name.to_string(),
        path: std::path::PathBuf::from(name),
        size,
        finish_ts,
    }
}

#[test]
fn pick_for_eviction_is_noop_when_cap_unset() {
    let candidates = vec![candidate("a", 100, 1)];
    assert!(pick_for_eviction(candidates, 0, 100).is_empty());
}

#[test]
fn pick_for_eviction_is_noop_when_under_cap() {
    let candidates = vec![candidate("a", 100, 1)];
    assert!(pick_for_eviction(candidates, 200, 100).is_empty());
}

#[test]
fn pick_for_eviction_is_noop_when_exactly_at_cap() {
    let candidates = vec![candidate("a", 100, 1)];
    assert!(pick_for_eviction(candidates, 100, 100).is_empty());
}

#[test]
fn pick_for_eviction_evicts_oldest_finished_first() {
    // Total 300 over a 150 cap: the oldest-finished (ts 1) must go first; once evicting it alone
    // (100 bytes) still leaves 200 > 150, the next-oldest (ts 2) also goes, dropping to 100 <= 150.
    // The newest (ts 3) is never touched.
    let candidates = vec![
        candidate("newest", 100, 3),
        candidate("oldest", 100, 1),
        candidate("middle", 100, 2),
    ];
    let chosen = pick_for_eviction(candidates, 150, 300);
    let names: Vec<&str> = chosen
        .iter()
        .map(|candidate| candidate.name.as_str())
        .collect();
    assert_eq!(names, vec!["oldest", "middle"]);
}

#[test]
fn pick_for_eviction_stops_as_soon_as_under_cap() {
    // Evicting just the oldest (200 bytes) already drops 500 total to 300 <= 300 cap, so the
    // second-oldest must be left alone even though it exists.
    let candidates = vec![candidate("oldest", 200, 1), candidate("newer", 100, 2)];
    let chosen = pick_for_eviction(candidates, 300, 500);
    assert_eq!(chosen.len(), 1);
    assert_eq!(chosen[0].name, "oldest");
}

#[test]
fn max_disk_bytes_defaults_to_zero_when_unset() {
    let prev = std::env::var_os(MAX_DISK_BYTES_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var(MAX_DISK_BYTES_ENV);
    }
    assert_eq!(max_disk_bytes(), 0);
    // SAFETY: single-threaded test execution.
    unsafe {
        match prev {
            Some(value) => std::env::set_var(MAX_DISK_BYTES_ENV, value),
            None => std::env::remove_var(MAX_DISK_BYTES_ENV),
        }
    }
}

#[test]
fn max_disk_bytes_parses_a_valid_value() {
    let prev = std::env::var_os(MAX_DISK_BYTES_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_DISK_BYTES_ENV, "1234");
    }
    assert_eq!(max_disk_bytes(), 1234);
    // SAFETY: single-threaded test execution.
    unsafe {
        match prev {
            Some(value) => std::env::set_var(MAX_DISK_BYTES_ENV, value),
            None => std::env::remove_var(MAX_DISK_BYTES_ENV),
        }
    }
}

#[test]
fn max_disk_bytes_falls_back_to_zero_on_garbage() {
    let prev = std::env::var_os(MAX_DISK_BYTES_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_DISK_BYTES_ENV, "not-a-number");
    }
    assert_eq!(max_disk_bytes(), 0);
    // SAFETY: single-threaded test execution.
    unsafe {
        match prev {
            Some(value) => std::env::set_var(MAX_DISK_BYTES_ENV, value),
            None => std::env::remove_var(MAX_DISK_BYTES_ENV),
        }
    }
}

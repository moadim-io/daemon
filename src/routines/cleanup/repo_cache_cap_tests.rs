#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn candidate(name: &str, size: u64, last_fetch: u64) -> EvictCandidate {
    EvictCandidate {
        name: name.to_string(),
        path: std::path::PathBuf::from(name),
        size,
        last_fetch,
    }
}

fn touch_mirror(parent: &std::path::Path, name: &str) {
    std::fs::create_dir_all(parent.join(name)).unwrap();
}

fn write_bytes(parent: &std::path::Path, name: &str, len: usize) {
    std::fs::write(parent.join(name).join("HEAD"), vec![b'x'; len]).unwrap();
}

/// A `last_fetch_for` that reports each mirror's last-fetch time from its directory name
/// (`"{label}-{ts}"`), isolating cap eviction ordering from real filesystem mtimes — mirroring
/// `enforce_disk_cap_tests`'s `finish_at_trigger` seam.
fn last_fetch_from_name(path: &std::path::Path) -> u64 {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let (_label, ts) = name.rsplit_once('-').unwrap();
    ts.parse().unwrap()
}

#[test]
fn max_repo_cache_bytes_defaults_to_zero_when_unset() {
    let prev = std::env::var_os(MAX_REPO_CACHE_DISK_BYTES_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var(MAX_REPO_CACHE_DISK_BYTES_ENV);
    }
    assert_eq!(max_repo_cache_bytes(), 0);
    // SAFETY: single-threaded test execution.
    unsafe {
        match prev {
            Some(value) => std::env::set_var(MAX_REPO_CACHE_DISK_BYTES_ENV, value),
            None => std::env::remove_var(MAX_REPO_CACHE_DISK_BYTES_ENV),
        }
    }
}

#[test]
fn max_repo_cache_bytes_parses_a_valid_value() {
    let prev = std::env::var_os(MAX_REPO_CACHE_DISK_BYTES_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_REPO_CACHE_DISK_BYTES_ENV, "1234");
    }
    assert_eq!(max_repo_cache_bytes(), 1234);
    // SAFETY: single-threaded test execution.
    unsafe {
        match prev {
            Some(value) => std::env::set_var(MAX_REPO_CACHE_DISK_BYTES_ENV, value),
            None => std::env::remove_var(MAX_REPO_CACHE_DISK_BYTES_ENV),
        }
    }
}

#[test]
fn max_repo_cache_bytes_falls_back_to_zero_on_garbage() {
    let prev = std::env::var_os(MAX_REPO_CACHE_DISK_BYTES_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_REPO_CACHE_DISK_BYTES_ENV, "not-a-number");
    }
    assert_eq!(max_repo_cache_bytes(), 0);
    // SAFETY: single-threaded test execution.
    unsafe {
        match prev {
            Some(value) => std::env::set_var(MAX_REPO_CACHE_DISK_BYTES_ENV, value),
            None => std::env::remove_var(MAX_REPO_CACHE_DISK_BYTES_ENV),
        }
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
fn pick_for_eviction_evicts_least_recently_fetched_first() {
    // Total 300 over a 150 cap: the least-recently-fetched (ts 1) must go first; once evicting it
    // alone (100 bytes) still leaves 200 > 150, the next-oldest (ts 2) also goes, dropping to
    // 100 <= 150. The most-recently-fetched (ts 3) is never touched.
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
    let candidates = vec![candidate("oldest", 200, 1), candidate("newer", 100, 2)];
    let chosen = pick_for_eviction(candidates, 300, 500);
    assert_eq!(chosen.len(), 1);
    assert_eq!(chosen[0].name, "oldest");
}

#[test]
fn prune_orphaned_is_noop_for_a_missing_dir() {
    let missing = std::env::temp_dir().join(format!(
        "moadim-repo-cache-prune-missing-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&missing);
    let stats = prune_orphaned(&missing, &HashSet::new());
    assert_eq!(stats, ReapStats::default());
}

#[test]
fn prune_orphaned_removes_mirrors_no_routine_references() {
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-prune-orphan-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    touch_mirror(&base, "still-referenced");
    write_bytes(&base, "still-referenced", 10);
    touch_mirror(&base, "orphaned");
    write_bytes(&base, "orphaned", 20);

    let mut referenced = HashSet::new();
    referenced.insert("still-referenced".to_string());
    let stats = prune_orphaned(&base, &referenced);

    assert_eq!(stats.removed, 1);
    assert_eq!(stats.freed_bytes, 20);
    assert!(base.join("still-referenced").exists());
    assert!(!base.join("orphaned").exists());

    std::fs::remove_dir_all(&base).unwrap();
}
include!("prune_orphaned_skips_a_stray_file_tests.rs");

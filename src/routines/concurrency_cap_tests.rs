#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

/// Restore `MOADIM_MAX_CONCURRENT_RUNS` to whatever it was before the test ran.
fn restore(prev: Option<std::ffi::OsString>) {
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        match prev {
            Some(value) => std::env::set_var(MAX_CONCURRENT_RUNS_ENV, value),
            None => std::env::remove_var(MAX_CONCURRENT_RUNS_ENV),
        }
    }
}

/// Point `MOADIM_HOME_OVERRIDE` at a fresh tempdir for the duration of the test, so
/// `max_concurrent_runs()`'s file-override lookup never touches the real `~/.config/moadim`.
/// Cleaned up (dir removed, env var unset) when the returned guard drops.
struct HomeGuard {
    dir: std::path::PathBuf,
}

impl HomeGuard {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "moadim-concurrency-cap-{tag}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", &dir) }
        Self { dir }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe { std::env::remove_var("MOADIM_HOME_OVERRIDE") }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn max_concurrent_runs_defaults_when_unset() {
    let _home = HomeGuard::new("defaults");
    let prev = std::env::var_os(MAX_CONCURRENT_RUNS_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var(MAX_CONCURRENT_RUNS_ENV);
    }
    assert_eq!(max_concurrent_runs(), DEFAULT_MAX_CONCURRENT_RUNS);
    restore(prev);
}

#[test]
fn max_concurrent_runs_parses_a_valid_value() {
    let _home = HomeGuard::new("valid");
    let prev = std::env::var_os(MAX_CONCURRENT_RUNS_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_CONCURRENT_RUNS_ENV, "9");
    }
    assert_eq!(max_concurrent_runs(), 9);
    restore(prev);
}

#[test]
fn max_concurrent_runs_falls_back_to_default_on_garbage() {
    let _home = HomeGuard::new("garbage");
    let prev = std::env::var_os(MAX_CONCURRENT_RUNS_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_CONCURRENT_RUNS_ENV, "not-a-number");
    }
    assert_eq!(max_concurrent_runs(), DEFAULT_MAX_CONCURRENT_RUNS);
    restore(prev);
}

#[test]
fn max_concurrent_runs_parses_zero_as_unlimited() {
    // `0` is a valid, meaningful value here (unbounded) — same convention as the disk-cap env
    // var — not rejected like an unparsable value.
    let _home = HomeGuard::new("zero");
    let prev = std::env::var_os(MAX_CONCURRENT_RUNS_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_CONCURRENT_RUNS_ENV, "0");
    }
    assert_eq!(max_concurrent_runs(), 0);
    restore(prev);
}

// ─── file-override precedence (issue #1155) ────────────────────────────────

#[test]
fn max_concurrent_runs_uses_file_override_when_env_unset() {
    let _home = HomeGuard::new("file-override");
    let prev = std::env::var_os(MAX_CONCURRENT_RUNS_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var(MAX_CONCURRENT_RUNS_ENV);
    }
    crate::machine::set_max_concurrent_runs_override(Some(4)).expect("write cap override");
    assert_eq!(max_concurrent_runs(), 4);
    restore(prev);
}

#[test]
fn max_concurrent_runs_prefers_env_over_file_override() {
    let _home = HomeGuard::new("env-over-file");
    let prev = std::env::var_os(MAX_CONCURRENT_RUNS_ENV);
    crate::machine::set_max_concurrent_runs_override(Some(4)).expect("write cap override");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_CONCURRENT_RUNS_ENV, "9");
    }
    assert_eq!(max_concurrent_runs(), 9);
    restore(prev);
}

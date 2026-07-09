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

#[test]
fn max_concurrent_runs_defaults_when_unset() {
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
    let prev = std::env::var_os(MAX_CONCURRENT_RUNS_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_CONCURRENT_RUNS_ENV, "not-a-number");
    }
    assert_eq!(max_concurrent_runs(), DEFAULT_MAX_CONCURRENT_RUNS);
    restore(prev);
}

#[test]
fn max_concurrent_runs_falls_back_to_default_on_zero() {
    // Unlike the disk-cap env var, `0` is not "unbounded" here — an unbounded fan-out is exactly
    // the bug this cap prevents, so `0` is rejected like any other unparsable value.
    let prev = std::env::var_os(MAX_CONCURRENT_RUNS_ENV);
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var(MAX_CONCURRENT_RUNS_ENV, "0");
    }
    assert_eq!(max_concurrent_runs(), DEFAULT_MAX_CONCURRENT_RUNS);
    restore(prev);
}

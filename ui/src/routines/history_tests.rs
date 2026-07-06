//! Host-side unit tests for the pure formatting helpers in [`super`]: the run-status badge
//! class/label mapping and the `fmt_run_duration` elapsed-time formatter. No DOM/wasm dependency
//! (mirrors the `refresh.rs` test conventions).

use super::*;

#[test]
fn run_status_class_covers_every_variant() {
    assert_eq!(run_status_class(RunStatus::Running), "run-status running");
    assert_eq!(run_status_class(RunStatus::Success), "run-status success");
    assert_eq!(run_status_class(RunStatus::Failed), "run-status failed");
    assert_eq!(run_status_class(RunStatus::Unknown), "run-status unknown");
}

#[test]
fn run_status_label_covers_every_variant() {
    assert_eq!(run_status_label(RunStatus::Running), "RUNNING");
    assert_eq!(run_status_label(RunStatus::Success), "SUCCESS");
    assert_eq!(run_status_label(RunStatus::Failed), "FAILED");
    assert_eq!(run_status_label(RunStatus::Unknown), "UNKNOWN");
}

#[test]
fn fmt_run_duration_under_a_minute_is_seconds() {
    assert_eq!(fmt_run_duration(1_000, 1_045), "45s");
}

#[test]
fn fmt_run_duration_exact_minute_boundary_is_minutes() {
    assert_eq!(fmt_run_duration(0, 60), "1m");
}

#[test]
fn fmt_run_duration_under_an_hour_is_minutes() {
    assert_eq!(fmt_run_duration(0, 754), "12m");
}

#[test]
fn fmt_run_duration_exact_hour_boundary_is_hours_and_minutes() {
    assert_eq!(fmt_run_duration(0, 3_600), "1h 0m");
}

#[test]
fn fmt_run_duration_over_an_hour_is_hours_and_minutes() {
    assert_eq!(fmt_run_duration(0, 7_530), "2h 5m");
}

#[test]
fn fmt_run_duration_saturates_when_finished_precedes_started() {
    // A clock skew or malformed record must not panic on underflow.
    assert_eq!(fmt_run_duration(100, 50), "0s");
}

#[test]
fn fmt_retention_under_a_minute_reads_under_a_minute() {
    assert_eq!(fmt_retention(1_000, 1_030), "expires in <1m");
}

#[test]
fn fmt_retention_under_an_hour_is_minutes() {
    assert_eq!(fmt_retention(0, 754), "expires in 12m");
}

#[test]
fn fmt_retention_over_an_hour_is_hours_and_minutes() {
    assert_eq!(fmt_retention(0, 7_530), "expires in 2h 5m");
}

#[test]
fn fmt_retention_at_deadline_reads_expired() {
    assert_eq!(fmt_retention(1_000, 1_000), "expired");
}

#[test]
fn fmt_retention_past_deadline_reads_expired() {
    // Cleanup runs on its own interval, so a due run can still be visible past its deadline.
    assert_eq!(fmt_retention(1_500, 1_000), "expired");
}

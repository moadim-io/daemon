#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;
use crate::sync::{record_crontab_sync_failure, reset_crontab_sync_status_for_tests, SyncError};
use crate::utils::time::now_secs;

#[test]
fn build_reports_ok_and_running() {
    let response = build(now_secs());
    assert_eq!(response.status, "ok");
    assert!(response.running);
}

#[test]
fn build_clamps_uptime_to_zero_on_backward_clock_skew() {
    // uptime_start in the future models the wall clock jumping backward after the server
    // started — saturating_sub must clamp to 0 instead of underflowing.
    let response = build(now_secs() + 10_000);
    assert_eq!(response.uptime_secs, 0);
}

#[test]
fn build_carries_version_and_machine() {
    let response = build(now_secs());
    assert_eq!(response.version, crate::build_info::VERSION);
    assert_eq!(response.git_sha, crate::build_info::GIT_SHA);
    assert_eq!(response.build_date, crate::build_info::BUILD_DATE);
    assert!(!response.machine.is_empty());
}

#[test]
fn build_surfaces_last_crontab_sync_failure() {
    reset_crontab_sync_status_for_tests();
    record_crontab_sync_failure(&SyncError::CrontabCommand(
        "crontab - timed out".to_string(),
    ));

    let response = build(now_secs());

    assert!(!response.crontab_sync.ok);
    assert_eq!(
        response.crontab_sync.last_error.as_deref(),
        Some("crontab: crontab - timed out")
    );
    assert!(response.crontab_sync.last_error_at.is_some());
    reset_crontab_sync_status_for_tests();
}

#[test]
fn build_reports_healthy_crontab_sync_after_reset() {
    reset_crontab_sync_status_for_tests();
    let response = build(now_secs());

    assert!(response.crontab_sync.ok);
    assert_eq!(response.crontab_sync.last_error, None);
    assert_eq!(response.crontab_sync.last_error_at, None);
}

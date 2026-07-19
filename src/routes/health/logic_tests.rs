#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;
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

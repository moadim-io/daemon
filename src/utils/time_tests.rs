#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use std::time::Duration;

#[test]
fn secs_since_epoch_clamps_pre_1970_clock_to_zero() {
    // A clock that reads before the Unix epoch must not panic.
    let before_epoch = SystemTime::UNIX_EPOCH - Duration::from_secs(60);
    assert_eq!(secs_since_epoch(before_epoch), 0);
}

#[test]
fn secs_since_epoch_returns_whole_seconds() {
    let moment = SystemTime::UNIX_EPOCH + Duration::from_millis(1_577_836_800_500);
    assert_eq!(secs_since_epoch(moment), 1_577_836_800);
}

#[test]
fn now_secs_after_year_2020() {
    // Unix timestamp for 2020-01-01T00:00:00Z
    assert!(now_secs() > 1_577_836_800);
}

#[test]
fn now_secs_before_year_2100() {
    // Unix timestamp for 2100-01-01T00:00:00Z
    assert!(now_secs() < 4_102_444_800);
}

#[test]
fn now_secs_is_non_decreasing() {
    let t1 = now_secs();
    let t2 = now_secs();
    assert!(t2 >= t1);
}

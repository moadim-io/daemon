#![allow(clippy::missing_docs_in_private_items)]

use super::*;

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

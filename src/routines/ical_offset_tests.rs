//! `format_utc_offset` tests, split out of `ical_tests.rs` to keep that file under the repo's
//! line-count gate.

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn utc_offset_formats_per_rfc5545() {
    assert_eq!(
        format_utc_offset(chrono::FixedOffset::east_opt(0).unwrap()),
        "+0000"
    );
    assert_eq!(
        format_utc_offset(chrono::FixedOffset::east_opt(3 * 3600).unwrap()),
        "+0300"
    );
    assert_eq!(
        format_utc_offset(chrono::FixedOffset::west_opt(5 * 3600).unwrap()),
        "-0500"
    );
    assert_eq!(
        format_utc_offset(chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap()),
        "+0530"
    );
    // A non-zero seconds component (some pre-1900 zones) appends a third HH:MM:SS group.
    assert_eq!(
        format_utc_offset(chrono::FixedOffset::east_opt(3 * 3600 + 25).unwrap()),
        "+030025"
    );
}

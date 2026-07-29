#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use std::path::Path;

use super::*;

// ─── Schedule conversion ───────────────────────────────────────────────────

#[test]
fn to_os_schedule_7field_drops_sec_and_year() {
    assert_eq!(to_os_schedule("0 30 9 * * 1-5 *"), "30 9 * * 1-5");
}

#[test]
fn to_os_schedule_6field_drops_seconds() {
    // 6-field `sec min hour dom month dow` -> 5-field. Without reduction the
    // expression is written verbatim to the OS crontab and never fires.
    assert_eq!(to_os_schedule("0 */5 * * * *"), "*/5 * * * *");
    assert_eq!(to_os_schedule("30 0 9 * * 1-5"), "0 9 * * 1-5");
    assert_eq!(to_os_schedule("0 30 9 * * 1-5"), "30 9 * * 1-5");
    assert_eq!(to_os_schedule("*/30 * * * * *"), "* * * * *");
}

#[test]
fn to_os_schedule_passthrough_keyword() {
    assert_eq!(to_os_schedule("@daily"), "@daily");
    assert_eq!(to_os_schedule("@reboot"), "@reboot");
    assert_eq!(to_os_schedule("@hourly"), "@hourly");
}

#[test]
fn to_os_schedule_5field_unchanged() {
    assert_eq!(to_os_schedule("30 9 * * 1-5"), "30 9 * * 1-5");
}

#[test]
fn to_os_schedule_trims_whitespace() {
    assert_eq!(to_os_schedule("  0 0 * * * * *  "), "0 * * * *");
}

#[test]
fn to_os_schedule_non_5_or_7_field_passthrough() {
    // Covers the `_ =>` arm: neither @keyword, 5-field, nor 7-field.
    assert_eq!(to_os_schedule("1 2 3"), "1 2 3");
    assert_eq!(to_os_schedule("a b c d e f g h"), "a b c d e f g h");
}

// ─── SyncError Display & From<io::Error> ─────────────────────────────────────

#[test]
fn sync_error_display_renders_both_variants() {
    let cmd = SyncError::CrontabCommand("nope".to_string());
    assert_eq!(format!("{cmd}"), "crontab: nope");

    let io_err = std::io::Error::other("disk gone");
    let wrapped = SyncError::Io(io_err);
    assert_eq!(format!("{wrapped}"), "io: disk gone");
}

#[test]
fn sync_error_from_io_error_wraps_io_variant() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let converted: SyncError = io_err.into();
    match converted {
        SyncError::Io(inner) => {
            assert_eq!(inner.kind(), std::io::ErrorKind::PermissionDenied);
        }
        SyncError::CrontabCommand(msg) => panic!("expected Io variant, got CrontabCommand({msg})"),
    }
}

// ─── crontab_bin test-build guard ────────────────────────────────────────────

#[test]
fn crontab_bin_never_resolves_to_real_crontab_in_test_builds() {
    // Structural guard for issue #175: in a test build, with no `MOADIM_CRONTAB_BIN`
    // shim configured, `crontab_bin()` must never fall back to the real `crontab`,
    // so a test that forgets to isolate the crontab cannot clobber the developer's
    // live crontab. The resolved path must also not exist, so the eventual spawn
    // fails harmlessly and the sync only logs a warning.
    let saved = std::env::var_os("MOADIM_CRONTAB_BIN");
    // SAFETY: single-threaded test harness (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var("MOADIM_CRONTAB_BIN");
    }
    let bin = crontab_bin();
    // SAFETY: single-threaded test execution.
    unsafe {
        match saved {
            Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
            None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
        }
    }
    assert_ne!(
        bin, "crontab",
        "test build must not fall back to the real crontab"
    );
    assert!(
        !Path::new(&bin).exists(),
        "the test-build crontab guard path must not exist so the spawn fails: {bin}"
    );
}

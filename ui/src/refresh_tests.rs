//! Host-side unit tests for the pure auto-refresh logic in [`super`]: the
//! `RefreshInterval` codec (token round-trip, millis, labels) and the
//! `fmt_freshness` staleness formatter. No DOM/wasm dependency (mirrors the
//! `schedule.rs` test conventions).

use super::*;

#[test]
fn token_roundtrips_for_every_variant() {
    for interval in RefreshInterval::ALL {
        assert_eq!(RefreshInterval::from_token(interval.to_token()), interval);
    }
}

#[test]
fn from_token_defaults_to_off_for_unknown() {
    assert_eq!(RefreshInterval::from_token(""), RefreshInterval::Off);
    assert_eq!(RefreshInterval::from_token("off"), RefreshInterval::Off);
    assert_eq!(
        RefreshInterval::from_token("nonsense"),
        RefreshInterval::Off
    );
    assert_eq!(RefreshInterval::from_token("7"), RefreshInterval::Off);
}

#[test]
fn default_is_off() {
    assert_eq!(RefreshInterval::default(), RefreshInterval::Off);
}

#[test]
fn off_has_no_cadence_others_do() {
    assert_eq!(RefreshInterval::Off.as_millis(), None);
    assert_eq!(RefreshInterval::S5.as_millis(), Some(5_000));
    assert_eq!(RefreshInterval::S15.as_millis(), Some(15_000));
    assert_eq!(RefreshInterval::S30.as_millis(), Some(30_000));
    assert_eq!(RefreshInterval::S60.as_millis(), Some(60_000));
}

#[test]
fn labels_are_stable() {
    assert_eq!(RefreshInterval::Off.label(), "Off");
    assert_eq!(RefreshInterval::S5.label(), "5s");
    assert_eq!(RefreshInterval::S15.label(), "15s");
    assert_eq!(RefreshInterval::S30.label(), "30s");
    assert_eq!(RefreshInterval::S60.label(), "60s");
}

#[test]
fn all_lists_every_variant_in_selector_order() {
    assert_eq!(
        RefreshInterval::ALL,
        [
            RefreshInterval::Off,
            RefreshInterval::S5,
            RefreshInterval::S15,
            RefreshInterval::S30,
            RefreshInterval::S60,
        ]
    );
}

#[test]
fn fmt_freshness_sub_minute_is_just_now() {
    assert_eq!(fmt_freshness(0), "updated just now");
    assert_eq!(fmt_freshness(59), "updated just now");
}

#[test]
fn fmt_freshness_minutes() {
    assert_eq!(fmt_freshness(60), "updated 1m ago");
    assert_eq!(fmt_freshness(3_599), "updated 59m ago");
}

#[test]
fn fmt_freshness_hours() {
    assert_eq!(fmt_freshness(3_600), "updated 1h ago");
    assert_eq!(fmt_freshness(7_200), "updated 2h ago");
}

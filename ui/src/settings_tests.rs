//! Host-side unit tests for the pure `parse_cap` helper in [`super`]. The rest of this module is
//! a Yew component (DOM-touching, wasm-only) and isn't host-testable.

use super::*;

#[test]
fn parse_cap_empty_is_none() {
    assert_eq!(parse_cap(""), None);
}

#[test]
fn parse_cap_whitespace_only_is_none() {
    assert_eq!(parse_cap("   "), None);
}

#[test]
fn parse_cap_parses_a_valid_number() {
    assert_eq!(parse_cap("5"), Some(5));
}

#[test]
fn parse_cap_trims_surrounding_whitespace() {
    assert_eq!(parse_cap("  9  "), Some(9));
}

#[test]
fn parse_cap_zero_is_some_zero() {
    // `0` is a meaningful, explicit value here (matches the daemon's `0` = unbounded
    // convention) — distinct from an empty field, which means "no override at all".
    assert_eq!(parse_cap("0"), Some(0));
}

#[test]
fn parse_cap_rejects_non_numeric() {
    assert_eq!(parse_cap("abc"), None);
}

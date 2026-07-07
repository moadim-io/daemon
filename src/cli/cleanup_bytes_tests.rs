//! Tests for the `cleanup` command's `freed_bytes` reporting: parsing it out of the server's
//! response body and rendering it as a human-readable size. Split out of `cli_tests.rs` to keep
//! that file under the repo's line-count gate.

use super::*;

#[test]
fn humanize_bytes_formats_units() {
    assert_eq!(humanize_bytes(0), "0 B");
    assert_eq!(humanize_bytes(512), "512 B");
    assert_eq!(humanize_bytes(1024), "1.0 KB");
    assert_eq!(humanize_bytes(12_400_000), "11.8 MB");
    // Caps at TB rather than indexing past the unit table, even for a value this large.
    assert_eq!(humanize_bytes(u64::MAX), "16777216.0 TB");
}

#[test]
fn parses_freed_bytes_from_cleanup_body() {
    assert_eq!(
        parse_freed_bytes("{\"removed\":1,\"freed_bytes\":4096}"),
        Some(4096)
    );
}

#[test]
fn freed_bytes_missing_degrades_to_none_for_older_server_bodies() {
    // A body from a server that predates the `freed_bytes` field (just `{"removed": N}`) must not
    // be treated as an error — the CLI falls back to 0 via `.unwrap_or(0)`.
    assert_eq!(parse_freed_bytes("{\"removed\":1}"), None);
    assert_eq!(parse_freed_bytes(""), None);
    assert_eq!(parse_freed_bytes("not json"), None);
}

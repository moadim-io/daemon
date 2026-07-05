use super::*;

// ── ics_feed_url ────────────────────────────────────────────────────────────────

#[test]
fn ics_feed_url_joins_origin_and_path() {
    assert_eq!(
        ics_feed_url("https://moadim.example.com"),
        "https://moadim.example.com/api/v1/routines.ics"
    );
}

#[test]
fn ics_feed_url_preserves_port() {
    assert_eq!(
        ics_feed_url("http://localhost:8787"),
        "http://localhost:8787/api/v1/routines.ics"
    );
}

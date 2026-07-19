use super::*;

// ── clone_title ───────────────────────────────────────────────────────────────

#[test]
fn clone_title_prepends_copy_of() {
    assert_eq!(clone_title("Daily report"), "Copy of Daily report");
}

#[test]
fn clone_title_does_not_double_prefix() {
    assert_eq!(clone_title("Copy of Daily report"), "Copy of Daily report");
}

#[test]
fn clone_title_preserves_empty_string() {
    assert_eq!(clone_title(""), "Copy of ");
}

// ── TTL_PRESETS ──────────────────────────────────────────────────────────────

#[test]
fn ttl_presets_map_labels_to_seconds() {
    assert_eq!(
        TTL_PRESETS,
        [
            ("3600", "1h"),
            ("86400", "1d"),
            ("604800", "7d"),
            ("2592000", "30d"),
        ]
    );
}

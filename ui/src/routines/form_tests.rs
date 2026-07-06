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

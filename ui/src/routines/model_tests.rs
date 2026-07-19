use super::*;

/// `GET /routines` omits `prompt` by default (see #825); the UI's hand-mirrored
/// `Routine` struct must tolerate that or every routines-list fetch fails (#849).
#[test]
fn routine_deserializes_without_prompt_field() {
    let json = r#"{
        "id": "r1",
        "schedule": "0 0 * * *",
        "title": "T",
        "agent": "a",
        "enabled": true
    }"#;
    let routine: Routine = serde_json::from_str(json).unwrap();
    assert_eq!(routine.prompt, "");
}

/// Mirrors the CLI's own `humanize_bytes` test (`src/cli/cleanup_bytes_tests.rs`) so the two
/// implementations stay provably identical: same unit boundaries, same rounding, same TB cap.
#[test]
fn humanize_bytes_formats_units() {
    assert_eq!(humanize_bytes(0), "0 B");
    assert_eq!(humanize_bytes(512), "512 B");
    assert_eq!(humanize_bytes(1024), "1.0 KB");
    assert_eq!(humanize_bytes(12_400_000), "11.8 MB");
    // Caps at TB rather than indexing past the unit table, even for a value this large.
    assert_eq!(humanize_bytes(u64::MAX), "16777216.0 TB");
}

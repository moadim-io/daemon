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

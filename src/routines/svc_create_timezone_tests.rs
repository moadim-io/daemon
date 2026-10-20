/// Build a create request with the given title and `timezone`, otherwise valid.
fn create_req_with_timezone(title: &str, timezone: Option<&str>) -> CreateRoutineRequest {
    CreateRoutineRequest {
        timezone: timezone.map(str::to_string),
        ..create_req_with_title(title)
    }
}

#[test]
fn svc_create_rejects_an_unknown_timezone_name() {
    let _home = TempHome::set();
    // Covers `validate_timezone`'s IANA-lookup reject branch: a string that merely looks like a
    // zone name, but is not present in the on-disk zoneinfo database, 400s rather than being
    // persisted and silently never applied (issue #405).
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone(
            "Svc Create Unknown Timezone ZZZ",
            Some("Definitely/Not_A_Zone"),
        ),
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_a_path_traversal_timezone_value() {
    let _home = TempHome::set();
    // A crafted value must not reach the filesystem check at all (issue #405).
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone(
            "Svc Create Traversal Timezone ZZZ",
            Some("../../etc/passwd"),
        ),
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_treats_a_blank_timezone_as_unset() {
    let _home = TempHome::set();
    // Mirrors `model`: a blank/whitespace-only value is not an error, it just means "no override".
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone("Svc Create Blank Timezone ZZZ", Some("   ")),
    )
    .expect("blank timezone should not be rejected");
    assert_eq!(result.routine.timezone, None);
}

#[test]
fn svc_create_accepts_a_valid_iana_timezone() {
    let _home = TempHome::set();
    // The in-process scheduler applies the override on every host, so every host accepts it.
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone("Svc Create Valid Timezone ZZZ", Some("Asia/Jerusalem")),
    )
    .expect("a real IANA zone name should be accepted");
    assert_eq!(result.routine.timezone.as_deref(), Some("Asia/Jerusalem"));
}

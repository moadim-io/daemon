
#[test]
fn svc_create_rejects_blank_title() {
    let _home = TempHome::set();
    // Covers the `reject_blank("title", ..)` error arm in `svc_create`: a
    // whitespace-only title is refused before any slug/disk work (#226).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
        disabled_reason: None,
            model: None,
            goal: None,
            title: "   ".into(),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_blank_prompt() {
    let _home = TempHome::set();
    // Covers the `reject_blank("prompt", ..)` error arm in `svc_create`: an empty
    // prompt would make the routine fire forever with no task (#224).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
        disabled_reason: None,
            model: None,
            goal: None,
            prompt: String::new(),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_zero_ttl_secs() {
    let _home = TempHome::set();
    // Covers the `reject_zero_secs("ttl_secs", ..)` error arm in `svc_create`:
    // a zero TTL reaps finished-run logs instantly (#233).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
        disabled_reason: None,
            model: None,
            goal: None,
            ttl_secs: Some(0),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_zero_max_runtime_secs() {
    let _home = TempHome::set();
    // Covers the `reject_zero_secs("max_runtime_secs", ..)` error arm in
    // `svc_create`: a zero cap self-kills the run immediately (#233).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
        disabled_reason: None,
            model: None,
            goal: None,
            max_runtime_secs: Some(0),
            power_saving_exempt: false,
            tags: vec![],
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_create_persists_machines_tests.rs"]
mod svc_create_persists_machines_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_create_clears_tombstone_so_a_deliberately_recreated_default_reseed_tests.rs"]
mod svc_create_clears_tombstone_so_a_deliberately_recreated_default_reseed_tests;

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn svc_update_disables_with_reason_and_exposes_it() {
    let _home = TempHome::set();
    let store = new_store();
    let routine = make_routine("disable-reason-id", "Svc Disable Reason ZZZ", 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("disable-reason-id".into(), routine);

    with_empty_path(|| {
        let mut req = empty_update_request();
        req.enabled = Some(false);
        req.disabled_reason = Some("Testing maintenance".to_string());

        let updated = svc_update(&store, "disable-reason-id", req).unwrap();

        assert!(!updated.routine.enabled);
        assert_eq!(
            updated.routine.disabled_reason.as_deref(),
            Some("Testing maintenance")
        );
    });
}

#[test]
fn svc_update_enable_clears_disabled_reason() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine(
        "enable-clears-reason-id",
        "Svc Enable Clears Reason ZZZ",
        1,
        1,
    );
    routine.enabled = false;
    routine.disabled_reason = Some("Old reason".to_string());
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("enable-clears-reason-id".into(), routine);

    with_empty_path(|| {
        let mut req = empty_update_request();
        req.enabled = Some(true);

        let updated = svc_update(&store, "enable-clears-reason-id", req).unwrap();

        assert!(updated.routine.enabled);
        assert_eq!(updated.routine.disabled_reason, None);
    });
}

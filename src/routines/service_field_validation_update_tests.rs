#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use super::service_field_validation_create_tests::{make_routine, TempHome};
use crate::routines::new_store;

#[test]
fn svc_update_rejects_blank_and_punctuation_titles() {
    let _home = TempHome::set();
    let original = "Svc Update Title Guard ZZZ";
    for title in ["", "   ", "!!!"] {
        let store = new_store();
        let routine = make_routine("title-guard-id", original, 1, 1);
        crate::routine_storage::write_routine(&routine).unwrap();
        store
            .lock()
            .unwrap()
            .insert("title-guard-id".into(), routine);

        let result = svc_update(
            &store,
            "title-guard-id",
            UpdateRoutineRequest {
                title: Some(title.into()),
                ..Default::default()
            },
        );
        assert!(
            matches!(result, Err(AppError::BadRequest(_))),
            "update to title {title:?} should be rejected"
        );
        assert_eq!(
            store.lock().unwrap().get("title-guard-id").unwrap().title,
            original
        );
    }
}

#[test]
fn svc_update_rejects_unknown_agent() {
    let _home = TempHome::set();
    let title = "Svc Update Unknown Agent ZZZ";
    let store = new_store();
    let routine = make_routine("upd-agent-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-agent-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-agent-id",
        UpdateRoutineRequest {
            agent: Some("no-such-agent-zzz".into()),
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert_eq!(
        store.lock().unwrap().get("upd-agent-id").unwrap().agent,
        "claude"
    );
}

#[test]
fn svc_update_rejects_blank_repository_url() {
    let _home = TempHome::set();
    let title = "Svc Update Blank Repo ZZZ";
    let store = new_store();
    let routine = make_routine("upd-repo-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-repo-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-repo-id",
        UpdateRoutineRequest {
            repositories: Some(vec![Repository {
                repository: " ".into(),
                branch: None,
                auto_pull: true,
            }]),
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store
        .lock()
        .unwrap()
        .get("upd-repo-id")
        .unwrap()
        .repositories
        .is_empty());
}

#[test]
fn svc_update_rejects_invalid_env_key() {
    let _home = TempHome::set();
    let title = "Svc Update Invalid Env Key ZZZ";
    let store = new_store();
    let routine = make_routine("upd-env-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-env-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-env-id",
        UpdateRoutineRequest {
            env: Some(std::collections::HashMap::from([(
                "not-valid".to_string(),
                "x".to_string(),
            )])),
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store
        .lock()
        .unwrap()
        .get("upd-env-id")
        .unwrap()
        .env
        .is_empty());
}

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::{new_store, slugify};

fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        auto_pull: true,
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

#[test]
fn svc_create_rejects_empty_prompt() {
    // Covers `validate_prompt`'s reject branch via `svc_create`: an empty prompt
    // is a 400 before any persistence or crontab sync (issue #224).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            auto_pull: true,
            model: None,
            schedule: "@daily".into(),
            title: "Svc Create Empty Prompt ZZZ".into(),
            agent: "claude".into(),
            prompt: String::new(),
            goal: None,
            repositories: vec![],
            machines: vec![],
            tags: vec![],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // No routine was created, so the store stays empty.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_whitespace_prompt() {
    // A whitespace-only prompt trims to empty and is rejected like a blank one.
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            auto_pull: true,
            model: None,
            schedule: "@daily".into(),
            title: "Svc Create Whitespace Prompt ZZZ".into(),
            agent: "claude".into(),
            prompt: "   \n\t".into(),
            goal: None,
            repositories: vec![],
            machines: vec![],
            tags: vec![],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_update_rejects_clearing_prompt_to_empty() {
    // Covers the `req.prompt` validation branch in `svc_update`: updating an
    // existing routine's prompt to whitespace-only is a 400, and the stored
    // prompt is left untouched (issue #224).
    let title = "Svc Update Empty Prompt ZZZ";
    let store = new_store();
    let routine = make_routine("empty-prompt-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("empty-prompt-id".into(), routine);

    let result = svc_update(
        &store,
        "empty-prompt-id",
        UpdateRoutineRequest {
            auto_pull: None,
            model: None,
            schedule: None,
            title: None,
            agent: None,
            prompt: Some("   ".into()),
            goal: None,
            repositories: None,
            machines: None,
            tags: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert_eq!(
        store.lock().unwrap().get("empty-prompt-id").unwrap().prompt,
        "do the thing"
    );

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

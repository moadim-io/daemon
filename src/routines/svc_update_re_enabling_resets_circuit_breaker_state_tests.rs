#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_update_re_enabling_resets_circuit_breaker_state() {
    let _home = TempHome::set();
    // Covers the `req.enabled == Some(true)` reset branch in `svc_update` (#521): a manual
    // re-enable must clear both the failure streak and the auto-disable reason, not just flip
    // `enabled` back on.
    let title = "Svc Update Reenable Resets Breaker ZZZ";
    let store = new_store();
    let mut routine = make_routine("reenable-id", title, 1, 1);
    routine.enabled = false;
    routine.consecutive_failures = 4;
    routine.auto_disabled_reason = Some("auto-disabled after 4 consecutive failed run(s)".into());
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("reenable-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "reenable-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                enabled: Some(true),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert!(updated.routine.enabled);
        assert_eq!(updated.routine.consecutive_failures, 0);
        assert!(updated.routine.auto_disabled_reason.is_none());
    });
}

#[test]
fn svc_update_disabling_does_not_touch_circuit_breaker_state() {
    let _home = TempHome::set();
    // The reset only fires for `enabled == Some(true)`; a manual disable (or an update that
    // doesn't touch `enabled` at all) must leave an in-progress failure streak alone.
    let title = "Svc Update Disable Keeps Breaker State ZZZ";
    let store = new_store();
    let mut routine = make_routine("disable-id", title, 1, 1);
    routine.consecutive_failures = 2;
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("disable-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "disable-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                enabled: Some(false),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert!(!updated.routine.enabled);
        assert_eq!(updated.routine.consecutive_failures, 2);
    });
}

#[test]
fn svc_update_trims_title_before_persisting() {
    // Covers the title `.trim()` on the `svc_update` apply path. Renaming with the
    // same slug but different spacing/case must store the trimmed title.
    let title = "Svc Update Trim ZZZ";
    let store = new_store();
    let routine = make_routine("trim-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("trim-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "trim-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                // Same slug, padded: applies the rename branch without a conflict.
                title: Some("  Svc Update Trim ZZZ  ".into()),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.title, title);
    });

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_update_title_does_not_move_filesystem_owned_folder() {
    let _home = TempHome::set();
    let rel = "team/ops/stable-dir";
    let store = new_store();
    let mut routine = make_routine("stable-dir-id", "Old Display Title", 1, 1);
    routine.prompt = "old prompt".to_string();
    let dir = crate::paths::routine_dir(rel);
    std::fs::create_dir_all(dir.join("prompts")).unwrap();
    std::fs::write(
        crate::paths::routine_toml_path(rel),
        "id = \"stable-dir-id\"\ntitle = \"Old Display Title\"\nagent = \"claude\"\n",
    )
    .unwrap();
    std::fs::write(crate::paths::routine_cron_path(rel), "@daily\n").unwrap();
    std::fs::write(crate::paths::routine_pure_prompt_path(rel), "old prompt").unwrap();
    store
        .lock()
        .unwrap()
        .insert("stable-dir-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "stable-dir-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                title: Some("New Display Title".into()),
                prompt: Some("new prompt".into()),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.title, "New Display Title");
        assert_eq!(updated.rel_path, rel);
    });

    assert!(crate::paths::routine_toml_path(rel).exists());
    assert_eq!(
        std::fs::read_to_string(crate::paths::routine_pure_prompt_path(rel)).unwrap(),
        "new prompt"
    );
    assert!(!crate::paths::routine_toml_path("new-display-title").exists());
}

#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_move_migrates_only_matching_workbenches_when_slug_changes() {
    let _home = TempHome::set();
    let routine = make_routine("move-workbench-id", "Old Slug");
    crate::routine_storage::write_routine(&routine).unwrap();
    let workbenches = crate::paths::workbenches_dir();
    crate::utils::fs_perms::create_private_dir_all(&workbenches.join("old-slug-111")).unwrap();
    crate::utils::fs_perms::create_private_dir_all(&workbenches.join("other-222")).unwrap();
    crate::utils::fs_perms::create_private_dir_all(&workbenches.join("new-slug-blocked")).unwrap();
    std::fs::write(workbenches.join("new-slug-blocked/file"), "occupied").unwrap();
    crate::utils::fs_perms::create_private_dir_all(&workbenches.join("old-slug-blocked")).unwrap();
    let store = Arc::new(Mutex::new(std::collections::HashMap::from([(
        routine.id.clone(),
        routine,
    )])));

    svc_move(
        &store,
        "move-workbench-id",
        MoveRoutineRequest {
            folder: None,
            slug: "new-slug".to_string(),
        },
    )
    .unwrap();

    assert!(workbenches.join("new-slug-111").exists());
    assert!(workbenches.join("other-222").exists());
    assert!(!workbenches.join("old-slug-111").exists());
    assert!(workbenches.join("old-slug-blocked").exists());
}

#[test]
fn svc_move_to_new_folder_with_same_slug_skips_workbench_migration() {
    let _home = TempHome::set();
    let routine = make_routine("move-same-slug-workbench-id", "Same Slug");
    crate::routine_storage::write_routine(&routine).unwrap();
    let workbenches = crate::paths::workbenches_dir();
    crate::utils::fs_perms::create_private_dir_all(&workbenches.join("same-slug-111")).unwrap();
    let store = Arc::new(Mutex::new(std::collections::HashMap::from([(
        routine.id.clone(),
        routine,
    )])));

    svc_move(
        &store,
        "move-same-slug-workbench-id",
        MoveRoutineRequest {
            folder: Some("team".to_string()),
            slug: "same-slug".to_string(),
        },
    )
    .unwrap();

    assert!(workbenches.join("same-slug-111").exists());
    assert!(crate::paths::routine_toml_path("team/same-slug").exists());
}

#[test]
fn svc_move_logs_and_succeeds_when_crontab_sync_fails() {
    let _home = TempHome::set();
    let _cron = FailingCronShim::set();
    let mut routine = make_routine("move-cron-id", "Cron Move");
    routine.machines = vec![crate::machine::current_machine()];
    crate::routine_storage::write_routine(&routine).unwrap();
    let store = Arc::new(Mutex::new(std::collections::HashMap::from([(
        routine.id.clone(),
        routine,
    )])));

    let response = svc_move(
        &store,
        "move-cron-id",
        MoveRoutineRequest {
            folder: None,
            slug: "cron-moved".to_string(),
        },
    )
    .unwrap();

    assert_eq!(response.rel_path, "cron-moved");
}

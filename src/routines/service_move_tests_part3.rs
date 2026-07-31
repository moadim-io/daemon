
#[test]
fn svc_move_rejects_absolute_or_parent_folder_paths() {
    let _home = TempHome::set();
    let routine = make_routine("move-bad-id", "Original Title");
    crate::routine_storage::write_routine(&routine).unwrap();
    let store = Arc::new(Mutex::new(std::collections::HashMap::from([(
        routine.id.clone(),
        routine,
    )])));

    for folder in [
        "/abs",
        "../escape",
        "team/../escape",
        "./escape",
        "team/./escape",
    ] {
        let result = svc_move(
            &store,
            "move-bad-id",
            MoveRoutineRequest {
                folder: Some(folder.to_string()),
                slug: "safe".to_string(),
            },
        );
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }
}

#[test]
fn svc_move_noops_when_target_matches_current_location() {
    let _home = TempHome::set();
    let routine = make_routine("move-same-id", "Same Title");
    crate::routine_storage::write_routine(&routine).unwrap();
    let store = Arc::new(Mutex::new(std::collections::HashMap::from([(
        routine.id.clone(),
        routine,
    )])));

    let response = svc_move(
        &store,
        "move-same-id",
        MoveRoutineRequest {
            folder: None,
            slug: "same-title".to_string(),
        },
    )
    .unwrap();

    assert_eq!(response.rel_path, "same-title");
}

#[test]
fn svc_move_rejects_invalid_slugs() {
    let _home = TempHome::set();
    let routine = make_routine("move-slug-id", "Slug Title");
    crate::routine_storage::write_routine(&routine).unwrap();
    let store = Arc::new(Mutex::new(std::collections::HashMap::from([(
        routine.id.clone(),
        routine,
    )])));

    for slug in ["", " ", ".", "..", "bad/slug", "bad\\slug"] {
        let result = svc_move(
            &store,
            "move-slug-id",
            MoveRoutineRequest {
                folder: None,
                slug: slug.to_string(),
            },
        );
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }
}

#[test]
fn svc_move_rejects_existing_target_directory() {
    let _home = TempHome::set();
    let routine = make_routine("move-conflict-id", "Source Routine");
    crate::routine_storage::write_routine(&routine).unwrap();
    crate::utils::fs_perms::create_private_dir_all(&crate::paths::routine_dir("taken/path"))
        .unwrap();
    let store = Arc::new(Mutex::new(std::collections::HashMap::from([(
        routine.id.clone(),
        routine,
    )])));

    let result = svc_move(
        &store,
        "move-conflict-id",
        MoveRoutineRequest {
            folder: Some("taken".to_string()),
            slug: "path".to_string(),
        },
    );

    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "service_move_tests_part2.rs"]
mod service_move_tests_part2;

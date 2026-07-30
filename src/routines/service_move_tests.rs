#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{Repository, Routine};
use std::sync::{Arc, Mutex};

struct TempHome {
    old_home: Option<std::ffi::OsString>,
    old_xdg: Option<std::ffi::OsString>,
}

impl TempHome {
    fn set() -> Self {
        let old_home = std::env::var_os("HOME");
        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let home = std::env::temp_dir().join(format!("moadim-move-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&home).unwrap();
        // SAFETY: these tests serialize their own temporary home setup and restore it in `Drop`.
        unsafe {
            std::env::set_var("HOME", &home);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        Self { old_home, old_xdg }
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: restore the process environment to the values captured before the test.
        unsafe {
            match &self.old_home {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
            match &self.old_xdg {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }
}

struct FailingCronShim {
    old_bin: Option<std::ffi::OsString>,
    dir: std::path::PathBuf,
}

impl FailingCronShim {
    fn set() -> Self {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("moadim-move-cron-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("crontab-shim.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nif [ \"$1\" = \"-l\" ]; then exit 0; fi\ncat > /dev/null\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let old_bin = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: restored by Drop; the repository test harness serializes env-mutating tests.
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { old_bin, dir }
    }
}

impl Drop for FailingCronShim {
    fn drop(&mut self) {
        // SAFETY: restore the process environment to the values captured before the test.
        unsafe {
            match &self.old_bin {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec!["@daily".to_string()],
        title: title.to_string(),
        agent: "claude".to_string(),
        model: None,
        prompt: "prompt".to_string(),
        goal: None,
        repositories: Vec::<Repository>::new(),
        machines: Vec::new(),
        enabled: true,
        source: "managed".to_string(),
        created_at: 1,
        updated_at: 1,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        consecutive_failures: 0,
        auto_disabled_reason: None,
        ttl_secs: None,
        max_runtime_secs: None,
        failure_threshold: None,
        tags: Vec::new(),
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn svc_move_moves_routine_directory_and_derives_new_location() {
    let _home = TempHome::set();
    let mut routine = make_routine("move-id", "Original Title");
    routine.prompt = "old prompt".to_string();
    crate::routine_storage::write_routine(&routine).unwrap();
    let store = Arc::new(Mutex::new(std::collections::HashMap::from([(
        routine.id.clone(),
        routine,
    )])));

    let response = svc_move(
        &store,
        "move-id",
        MoveRoutineRequest {
            folder: Some("team/ops".to_string()),
            slug: "nightly-check".to_string(),
        },
    )
    .unwrap();

    assert_eq!(response.folder.as_deref(), Some("team/ops"));
    assert_eq!(response.slug, "nightly-check");
    assert_eq!(response.rel_path, "team/ops/nightly-check");
    assert!(crate::paths::routine_toml_path("team/ops/nightly-check").exists());
    assert!(!crate::paths::routine_toml_path("original-title").exists());
}

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

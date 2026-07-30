#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::RoutineResponse;

fn scratch_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-routine-location-{}", uuid::Uuid::new_v4()))
}

fn with_override_home(body: impl FnOnce()) {
    let home = scratch_dir();
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body();
    // SAFETY: tests in this crate run single-threaded.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn write_routine_preserves_existing_filesystem_location_after_title_change() {
    with_override_home(|| {
        let id = "location-preserve-id";
        let original_rel = "hermes/learning/original-slug";
        let dir = crate::paths::routine_dir(original_rel);
        std::fs::create_dir_all(dir.join("prompts")).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(original_rel),
            format!("id = \"{id}\"\ntitle = \"Original title\"\nagent = \"claude\"\n"),
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(original_rel), "@daily\n").unwrap();
        std::fs::write(
            crate::paths::routine_pure_prompt_path(original_rel),
            "old task",
        )
        .unwrap();

        let mut routine = load_routine_from_dir(original_rel).unwrap();
        routine.title = "Renamed Display Title".to_string();
        routine.prompt = "new task".to_string();
        write_routine(&routine).unwrap();

        assert!(crate::paths::routine_toml_path(original_rel).exists());
        assert_eq!(
            std::fs::read_to_string(crate::paths::routine_pure_prompt_path(original_rel)).unwrap(),
            "new task"
        );
        assert!(
            !crate::paths::routine_toml_path("renamed-display-title").exists(),
            "title changes must not move filesystem-owned folders"
        );
    });
}

#[test]
fn routine_response_derives_folder_slug_and_paths_from_disk_location() {
    with_override_home(|| {
        let id = "location-response-id";
        let rel = "guidde/prs/review-nudge";
        let dir = crate::paths::routine_dir(rel);
        std::fs::create_dir_all(dir.join("prompts")).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(rel),
            format!("id = \"{id}\"\ntitle = \"Review Nudge\"\nagent = \"claude\"\n"),
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(rel), "@daily\n").unwrap();
        std::fs::write(crate::paths::routine_pure_prompt_path(rel), "task").unwrap();

        let response = RoutineResponse::from_routine(load_routine_from_dir(rel).unwrap());

        assert_eq!(response.folder.as_deref(), Some("guidde/prs"));
        assert_eq!(response.slug, "review-nudge");
        assert_eq!(response.rel_path, rel);
        assert!(response
            .file_path
            .ends_with("guidde/prs/review-nudge/routine.toml"));
    });
}

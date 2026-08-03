#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Routine};

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-disabled-{tag}-{}", uuid::Uuid::new_v4()))
}

fn with_override_home(body: impl FnOnce(&std::path::Path)) {
    let home = scratch_dir("override-home");
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body(&home);
    // SAFETY: tests in this crate run single-threaded per binary.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

fn make_routine(id: &str, title: &str, enabled: bool) -> Routine {
    crate::test_fixtures::routine_fixture(id, title)
        .enabled(enabled)
        .build()
}

#[test]
fn disabled_routine_writes_disabled_json_instead_of_enabled_toml() {
    with_override_home(|_home| {
        let title = "Rs Disabled Json Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-disabled-json-id", title, false)).unwrap();

        let disabled_text = std::fs::read_to_string(crate::paths::routine_disabled_json_path(&slug))
            .expect("disabled.json should be written for disabled routines");
        let disabled: serde_json::Value = serde_json::from_str(&disabled_text).unwrap();
        assert_eq!(disabled["version"], 1);
        assert_eq!(disabled["disabled_by_machine"], crate::machine::current_machine());
        assert_eq!(disabled["source"], "daemon");
        assert!(disabled["disabled_at"].as_str().is_some());
        assert!(disabled.get("reason").is_none());

        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("enabled"),
            "routine.toml must not carry enabled state"
        );
        assert!(!load_routine_from_dir(&slug).unwrap().enabled);
    });
}

#[test]
fn enabling_routine_removes_disabled_json() {
    with_override_home(|_home| {
        let title = "Rs Enable Removes Disabled Json";
        let slug = slugify(title);
        write_routine(&make_routine("rs-enable-removes-disabled-json-id", title, false)).unwrap();
        assert!(crate::paths::routine_disabled_json_path(&slug).exists());

        write_routine(&make_routine("rs-enable-removes-disabled-json-id", title, true)).unwrap();

        assert!(!crate::paths::routine_disabled_json_path(&slug).exists());
        assert!(load_routine_from_dir(&slug).unwrap().enabled);
    });
}

#[test]
fn disabled_json_presence_wins_over_legacy_toml_enabled_true_even_when_invalid() {
    with_override_home(|_home| {
        let title = "Rs Disabled Json Wins";
        let slug = slugify(title);
        write_routine(&make_routine("rs-disabled-json-wins-id", title, true)).unwrap();
        std::fs::write(crate::paths::routine_disabled_json_path(&slug), b"not json").unwrap();

        assert!(!load_routine_from_dir(&slug).unwrap().enabled);
    });
}

#[test]
fn legacy_toml_enabled_false_still_loads_as_disabled_without_disabled_json() {
    with_override_home(|_home| {
        let title = "Rs Legacy Toml Disabled";
        let slug = slugify(title);
        write_routine(&make_routine("rs-legacy-toml-disabled-id", title, true)).unwrap();
        let toml_path = crate::paths::routine_toml_path(&slug);
        let toml_text = std::fs::read_to_string(&toml_path).unwrap();
        std::fs::write(
            toml_path,
            toml_text.replace("agent = \"claude\"", "agent = \"claude\"\nenabled = false"),
        )
        .unwrap();

        assert!(!load_routine_from_dir(&slug).unwrap().enabled);
    });
}

#[test]
fn write_disabled_state_reports_remove_failure_when_marker_path_is_directory() {
    with_override_home(|_home| {
        let slug = "disabled-marker-directory";
        std::fs::create_dir_all(crate::paths::routine_disabled_json_path(slug)).unwrap();

        let err = write_disabled_state(slug, true, None).unwrap_err();

        assert!(matches!(
            err.kind(),
            std::io::ErrorKind::IsADirectory | std::io::ErrorKind::PermissionDenied
        ));
    });
}

#[test]
fn write_disabled_state_reports_atomic_write_failure_when_marker_path_is_directory() {
    with_override_home(|_home| {
        let slug = "disabled-marker-write-directory";
        std::fs::create_dir_all(crate::paths::routine_disabled_json_path(slug)).unwrap();

        let err = write_disabled_state(slug, false, None).unwrap_err();

        assert!(matches!(
            err.kind(),
            std::io::ErrorKind::IsADirectory | std::io::ErrorKind::AlreadyExists
        ));
    });
}

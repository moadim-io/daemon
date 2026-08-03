#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Routine};

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-disabled-reason-{tag}-{}", uuid::Uuid::new_v4()))
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
fn disabled_routine_writes_optional_reason_to_disabled_json() {
    with_override_home(|_home| {
        let title = "Rs Disabled Json Reason";
        let slug = slugify(title);
        let routine = crate::test_fixtures::routine_fixture("rs-disabled-json-reason-id", title)
            .enabled(false)
            .disabled_reason("Temporarily noisy")
            .build();

        write_routine(&routine).unwrap();

        let disabled_text = std::fs::read_to_string(crate::paths::routine_disabled_json_path(&slug))
            .expect("disabled.json should be written for disabled routines");
        let disabled: serde_json::Value = serde_json::from_str(&disabled_text).unwrap();
        assert_eq!(disabled["reason"], "Temporarily noisy");
        assert_eq!(
            load_routine_from_dir(&slug).unwrap().disabled_reason.as_deref(),
            Some("Temporarily noisy")
        );
    });
}

#[test]
fn old_disabled_json_without_reason_loads_with_no_disabled_reason() {
    with_override_home(|_home| {
        let title = "Rs Disabled Json Old Metadata";
        let slug = slugify(title);
        write_routine(&make_routine("rs-disabled-json-old-metadata-id", title, false)).unwrap();
        std::fs::write(
            crate::paths::routine_disabled_json_path(&slug),
            r#"{"version":1,"disabled_at":"2026-08-03T12:34:56Z","source":"daemon"}"#,
        )
        .unwrap();

        let loaded = load_routine_from_dir(&slug).unwrap();
        assert!(!loaded.enabled);
        assert_eq!(loaded.disabled_reason, None);
    });
}

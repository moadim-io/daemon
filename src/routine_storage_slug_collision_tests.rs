#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Repository, Routine};

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-rs-{tag}-{}", uuid::Uuid::new_v4()))
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
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        goal: None,
        repositories: vec![Repository {
            repository: "https://example.com/r.git".to_string(),
            branch: Some("main".to_string()),
        }],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 5,
        updated_at: 6,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn write_routine_rejects_slug_collision_with_a_different_id() {
    // Two distinct titles that slugify to the same folder name (#188) must not let the second
    // write silently clobber the first routine's on-disk files.
    with_override_home(|_home| {
        let title = "Update deps!";
        let other_title = "Update deps?";
        assert_eq!(slugify(title), slugify(other_title));

        write_routine(&make_routine("rs-collision-a", title)).unwrap();
        let err = write_routine(&make_routine("rs-collision-b", other_title)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);

        // The first routine's file must be untouched.
        let slug = slugify(title);
        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.id, "rs-collision-a");
    });
}

#[test]
fn write_routine_allows_rewriting_its_own_slug() {
    // The same routine (same id) writing to its own slug again — e.g. an update that doesn't
    // change the title — must not trip the collision guard.
    with_override_home(|_home| {
        let title = "Rs Rewrite Routine";
        let mut routine = make_routine("rs-rewrite-id", title);
        write_routine(&routine).unwrap();
        routine.updated_at = 99;
        write_routine(&routine).unwrap();

        let loaded = load_routine_from_dir(&slugify(title)).unwrap();
        assert_eq!(loaded.updated_at, 99);
    });
}

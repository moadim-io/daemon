#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn cleanup_expired_workbenches_prunes_the_reaped_workbench_claude_json_entry() {
    // Reaping a workbench must also prune its `projects[<workbench>]` entry from `~/.claude.json`
    // (issue #430), so that shared config does not accumulate one dead entry per reaped run. Drives
    // the public entry point end-to-end: a real `~/.claude.json` under the overridden home, seeded
    // with an entry for the workbench that is about to be reaped plus an unrelated entry that must
    // survive.
    let home = std::env::temp_dir().join(format!(
        "moadim-cleanup-claude-json-{}",
        uuid::Uuid::new_v4()
    ));
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    std::fs::create_dir_all(workbenches.join("orphan-1")).unwrap();
    let reaped_workbench = workbenches.join("orphan-1");
    let reaped_key = reaped_workbench.to_string_lossy().into_owned();

    let claude_json = crate::paths::claude_json_path().unwrap();
    std::fs::write(
        &claude_json,
        format!(
            r#"{{"projects":{{"{reaped_key}":{{"hasTrustDialogAccepted":true}},"/kept/wb":{{}}}}}}"#
        ),
    )
    .unwrap();

    let store = super::super::model::new_store();
    let removed = cleanup_expired_workbenches(&store);

    assert!(removed >= 1, "expected the orphan to be reaped");
    assert!(!reaped_workbench.exists());
    let rewritten: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    let projects = rewritten.get("projects").unwrap().as_object().unwrap();
    assert!(
        !projects.contains_key(&reaped_key),
        "the reaped workbench's projects entry must be pruned"
    );
    assert!(
        projects.contains_key("/kept/wb"),
        "unrelated projects entries must survive"
    );

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn cleanup_expired_workbenches_logs_and_continues_on_malformed_claude_json() {
    // A malformed `~/.claude.json` makes `prune_project` return `Err`, exercising
    // `prune_claude_json`'s error-logging arm — the reap itself must still proceed.
    let home = std::env::temp_dir().join(format!(
        "moadim-cleanup-claude-json-malformed-{}",
        uuid::Uuid::new_v4()
    ));
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    std::fs::create_dir_all(workbenches.join("orphan-1")).unwrap();
    let reaped_workbench = workbenches.join("orphan-1");

    let claude_json = crate::paths::claude_json_path().unwrap();
    std::fs::write(&claude_json, "not json").unwrap();

    let store = super::super::model::new_store();
    let removed = cleanup_expired_workbenches(&store);

    assert!(removed >= 1, "expected the orphan to be reaped");
    assert!(!reaped_workbench.exists());
    assert_eq!(
        std::fs::read_to_string(&claude_json).unwrap(),
        "not json",
        "a prune failure must not touch the malformed file"
    );

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

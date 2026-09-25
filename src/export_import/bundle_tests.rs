#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and cases do not need doc comments"
)]

use std::path::Path;

use super::{is_tracked_rel_path, Bundle, BUNDLE_VERSION};

fn tracked(rel: &str) -> bool {
    is_tracked_rel_path(Path::new(rel))
}

#[test]
fn tracked_global_files_are_accepted() {
    assert!(tracked("notifications.toml"));
    assert!(tracked("user_prompt.md"));
}

#[test]
fn untracked_global_files_are_rejected() {
    assert!(!tracked("machine.local.toml"));
    assert!(!tracked("daemon.log"));
    assert!(!tracked("moadim.pid"));
    assert!(!tracked("README.md"));
    assert!(!tracked(".gitignore"));
}

#[test]
fn agent_tomls_are_accepted_but_local_and_readme_are_not() {
    assert!(tracked("agents/claude.toml"));
    assert!(!tracked("agents/claude.local.toml"));
    assert!(!tracked("agents/README.md"));
    assert!(!tracked("agents/nested/claude.toml"));
}

#[test]
fn tracked_routine_files_are_accepted() {
    assert!(tracked("routines/my-routine/routine.toml"));
    assert!(tracked("routines/my-routine/schedule.cron"));
    assert!(tracked("routines/my-routine/disabled.json"));
    assert!(tracked("routines/my-routine/prompts/prompt.pure.md"));
    // Foldered routines nest the routine directory under grouping folders.
    assert!(tracked("routines/team/ops/nightly/routine.toml"));
    assert!(tracked("routines/team/ops/nightly/prompts/prompt.pure.md"));
}

#[test]
fn routine_runtime_sidecars_are_rejected() {
    assert!(!tracked("routines/my-routine/state.local.toml"));
    assert!(!tracked("routines/my-routine/routine.local.toml"));
    assert!(!tracked("routines/my-routine/schedule.compailed.cron"));
    assert!(!tracked("routines/my-routine/runs.log"));
    assert!(!tracked(
        "routines/my-routine/prompts/prompt.compiled.local.md"
    ));
    assert!(!tracked("routines/README.md"));
}

#[test]
fn misplaced_tracked_names_are_rejected() {
    // Tracked file names only count in their expected position.
    assert!(!tracked("routines/routine.toml"));
    assert!(!tracked("routine.toml"));
    assert!(!tracked("routines/my-routine/prompt.pure.md"));
    assert!(!tracked("routines/prompts/prompt.pure.md"));
}

#[test]
fn unsafe_paths_are_rejected() {
    assert!(!tracked("../outside/routine.toml"));
    assert!(!tracked("routines/../../etc/passwd"));
    assert!(!tracked("/etc/passwd"));
    // `.` segments are normalized away by `Path::components()`, which is harmless: the
    // resolved path still lands on the same tracked file inside the config dir.
    assert!(tracked("routines/./my-routine/routine.toml"));
}

#[test]
fn bundle_round_trips_through_json() {
    let mut files = std::collections::BTreeMap::new();
    files.insert("notifications.toml".to_string(), "[]".to_string());
    let bundle = Bundle {
        version: BUNDLE_VERSION,
        files,
    };
    let json = serde_json::to_string(&bundle).unwrap();
    let parsed: Bundle = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.version, BUNDLE_VERSION);
    assert_eq!(
        parsed.files.get("notifications.toml").map(String::as_str),
        Some("[]")
    );
}

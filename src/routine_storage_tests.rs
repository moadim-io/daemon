#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::{Repository, Routine};

fn make_routine(id: &str) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: "Store Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        repositories: vec![Repository {
            repository: "https://example.com/r.git".to_string(),
            branch: Some("main".to_string()),
        }],
        enabled: true,
        source: "managed".to_string(),
        created_at: 5,
        updated_at: 6,
        last_triggered_at: None,
    }
}

#[test]
fn write_then_load_round_trips() {
    let id = "rs-roundtrip";
    let routine = make_routine(id);
    write_routine(&routine).unwrap();

    assert!(crate::paths::routine_toml_path(id).exists());
    assert!(crate::paths::routine_prompt_path(id).exists());
    assert!(crate::paths::routine_gitignore_path(id).exists());

    let loaded = load_routine_from_dir(id).unwrap();
    assert_eq!(loaded.schedule, "@daily");
    assert_eq!(loaded.title, "Store Routine");
    assert_eq!(loaded.agent, "claude");
    assert_eq!(loaded.prompt, "task");
    assert_eq!(loaded.repositories.len(), 1);
    assert_eq!(loaded.repositories[0].branch.as_deref(), Some("main"));
    assert!(loaded.enabled);

    remove_routine_dir(id).unwrap();
    assert!(!crate::paths::routine_dir(id).exists());
}

#[test]
fn prompt_file_contains_composed_prompt() {
    let id = "rs-prompt";
    write_routine(&make_routine(id)).unwrap();
    let prompt = std::fs::read_to_string(crate::paths::routine_prompt_path(id)).unwrap();
    assert!(prompt.contains("# Workbench"));
    assert!(prompt.contains("https://example.com/r.git (branch main)"));
    assert!(prompt.contains("task"));
    remove_routine_dir(id).unwrap();
}

#[test]
fn load_routine_from_dir_missing_returns_none() {
    assert!(load_routine_from_dir("rs-does-not-exist-zzz").is_none());
}

#[test]
fn load_store_includes_written_routine() {
    let id = "rs-loadstore";
    write_routine(&make_routine(id)).unwrap();
    let store = load_store();
    assert!(store.lock().unwrap().contains_key(id));
    remove_routine_dir(id).unwrap();
}

#[test]
fn load_store_from_dir_missing_dir_empty() {
    let store = load_store_from_dir(std::path::Path::new("/nonexistent-routines-dir-99999"));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn remove_routine_dir_noop_when_absent() {
    remove_routine_dir("rs-never-created-zzz").unwrap();
}

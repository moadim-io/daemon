#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::{slugify, Repository, Routine};

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
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
        last_manual_trigger_at: None,
        ttl_secs: None,
    }
}

#[test]
fn write_then_load_round_trips() {
    let id = "rs-roundtrip-id";
    let title = "Rs Roundtrip Routine";
    let slug = slugify(title);
    let routine = make_routine(id, title);
    write_routine(&routine).unwrap();

    assert!(crate::paths::routine_toml_path(&slug).exists());
    assert!(crate::paths::routine_prompt_path(&slug).exists());
    assert!(crate::paths::routine_gitignore_path(&slug).exists());

    let loaded = load_routine_from_dir(&slug).unwrap();
    assert_eq!(loaded.id, id);
    assert_eq!(loaded.schedule, "@daily");
    assert_eq!(loaded.title, title);
    assert_eq!(loaded.agent, "claude");
    assert_eq!(loaded.prompt, "task");
    assert_eq!(loaded.repositories.len(), 1);
    assert_eq!(loaded.repositories[0].branch.as_deref(), Some("main"));
    assert!(loaded.enabled);

    remove_routine_dir(&slug).unwrap();
    assert!(!crate::paths::routine_dir(&slug).exists());
}

#[test]
fn prompt_file_contains_composed_prompt() {
    let title = "Rs Prompt Routine";
    let slug = slugify(title);
    write_routine(&make_routine("rs-prompt-id", title)).unwrap();
    let prompt = std::fs::read_to_string(crate::paths::routine_prompt_path(&slug)).unwrap();
    assert!(prompt.contains("# Workbench"));
    assert!(prompt.contains("https://example.com/r.git (branch main)"));
    assert!(prompt.contains("task"));
    remove_routine_dir(&slug).unwrap();
}

#[test]
fn load_routine_from_dir_missing_returns_none() {
    assert!(load_routine_from_dir("rs-does-not-exist-zzz").is_none());
}

#[test]
fn torn_routine_toml_loads_as_none() {
    // A truncated/garbage routine.toml (e.g. left by a crash mid-write) must not panic or load a
    // half-baked routine; the loader returns None and the routine is simply absent.
    let slug = "rs-torn-toml-routine";
    let dir = crate::paths::routine_dir(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(crate::paths::routine_toml_path(slug), "id = \"x\"\nschedu").unwrap();
    assert!(load_routine_from_dir(slug).is_none());
    remove_routine_dir(slug).unwrap();
}

#[test]
fn write_routine_leaves_no_tmp_residue() {
    let id = "rs-no-residue-id";
    let title = "Rs No Residue Routine";
    let slug = slugify(title);
    write_routine(&make_routine(id, title)).unwrap();
    let residue = std::fs::read_dir(crate::paths::routine_dir(&slug))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        .count();
    assert_eq!(residue, 0, "atomic_write must leave no .tmp files behind");
    remove_routine_dir(&slug).unwrap();
}

#[test]
fn load_store_includes_written_routine() {
    let id = "rs-loadstore-id";
    let title = "Rs Loadstore Routine";
    let slug = slugify(title);
    write_routine(&make_routine(id, title)).unwrap();
    let store = load_store();
    assert!(store.lock().unwrap().contains_key(id));
    remove_routine_dir(&slug).unwrap();
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

#[test]
fn migrate_routine_dirs_moves_legacy_uuid_dir_to_slug() {
    let id = "rs-legacy-uuid-1234";
    let title = "Rs Legacy Migrate Routine";
    let slug = slugify(title);
    let legacy_dir = crate::paths::routine_dir(id);
    std::fs::create_dir_all(&legacy_dir).unwrap();
    // Legacy layout: routine.toml + prompt.md live under the UUID-named dir.
    let toml = format!(
        "id = \"{id}\"\nschedule = \"@daily\"\ntitle = \"{title}\"\nagent = \"claude\"\nprompt = \"task\"\nenabled = true\n"
    );
    std::fs::write(legacy_dir.join("routine.toml"), toml).unwrap();
    std::fs::write(legacy_dir.join("prompt.md"), "legacy prompt").unwrap();

    migrate_routine_dirs();

    // Legacy dir removed; canonical slug dir now holds toml + prompt.
    assert!(!legacy_dir.exists(), "legacy UUID dir should be removed");
    assert!(crate::paths::routine_toml_path(&slug).exists());
    assert!(crate::paths::routine_prompt_path(&slug).exists());
    let loaded = load_routine_from_dir(&slug).unwrap();
    assert_eq!(loaded.id, id, "UUID id preserved across the dir migration");

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn repersist_routines_recreates_missing_prompt_sidecar() {
    let id = "rs-repersist-id";
    let title = "Rs Repersist Routine";
    let slug = slugify(title);
    write_routine(&make_routine(id, title)).unwrap();
    // Simulate the sync-only state: prompt.md gone, only run.sh-style dir remains.
    std::fs::remove_file(crate::paths::routine_prompt_path(&slug)).unwrap();
    assert!(!crate::paths::routine_prompt_path(&slug).exists());

    let mut map = HashMap::new();
    map.insert(id.to_string(), make_routine(id, title));
    let store = Arc::new(Mutex::new(map));
    repersist_routines(&store);

    assert!(
        crate::paths::routine_prompt_path(&slug).exists(),
        "repersist should recreate the prompt sidecar"
    );
    remove_routine_dir(&slug).unwrap();
}

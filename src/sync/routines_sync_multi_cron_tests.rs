#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{new_store, slugify, Routine};

fn make_routine(id: &str, title: &str, agent: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "30 9 * * 1-5".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: agent.to_string(),
        prompt: "p".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
    }
}

#[test]
fn build_block_reads_multiple_crons_from_schedule_sidecar_and_unions_redundant_entries() {
    let agent_name = "test-sync-agent-multi-cron-block";
    let title = "Multi Cron Sync Routine";
    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();
    let routine_dir = crate::paths::routine_dir(&slug);
    std::fs::create_dir_all(&routine_dir).unwrap();
    std::fs::write(
        routine_dir.join("schedule.cron"),
        "# human-edited extra fires\n*/10 * * * *\n*/20 * * * *\n5 9 * * 1-5\n",
    )
    .unwrap();
    std::fs::write(routine_dir.join(".compailed.cron"), "stale compiled cron\n").unwrap();

    let store = new_store();
    store.lock().unwrap().insert(
        "multi-cron".into(),
        make_routine("multi-cron", title, agent_name),
    );

    let block = build_block(&store);

    assert_eq!(
        block.matches("# moadim-routine:multi-cron").count(),
        2,
        "{block}"
    );
    assert!(block.contains("*/10 * * * * "), "{block}");
    assert!(block.contains("5 9 * * 1-5 "), "{block}");
    assert!(
        !block.contains("*/20 * * * * "),
        "cron-union should remove redundant subset: {block}"
    );
    assert_eq!(
        std::fs::read_to_string(crate::paths::routine_compailed_cron_path(&slug)).unwrap(),
        "*/10 * * * *\n5 9 * * 1-5\n"
    );
    assert!(
        !routine_dir.join(".compailed.cron").exists(),
        "legacy hidden cron-union sidecar should be removed"
    );
    assert_eq!(
        std::fs::read_to_string(routine_dir.join("schedule.cron")).unwrap(),
        "# human-edited extra fires\n*/10 * * * *\n*/20 * * * *\n5 9 * * 1-5\n"
    );

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(routine_dir);
}

#[test]
fn build_block_skips_invalid_multi_cron_entries_and_falls_back_if_none_valid() {
    let agent_name = "test-sync-agent-invalid-multi-cron-block";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let valid_title = "Invalid Multi Cron Valid Fallback Routine";
    let valid_slug = slugify(valid_title);
    let valid_dir = crate::paths::routine_dir(&valid_slug);
    std::fs::create_dir_all(&valid_dir).unwrap();
    std::fs::write(valid_dir.join("schedule.cron"), "not a cron\n5 9 * * 1-5\n").unwrap();

    let invalid_title = "Invalid Multi Cron All Invalid Routine";
    let invalid_slug = slugify(invalid_title);
    let invalid_dir = crate::paths::routine_dir(&invalid_slug);
    std::fs::create_dir_all(&invalid_dir).unwrap();
    std::fs::write(
        invalid_dir.join("schedule.cron"),
        "not a cron\n99 99 99 99 99\n",
    )
    .unwrap();

    let mut fallback = make_routine("all-invalid", invalid_title, agent_name);
    fallback.schedule = "15 10 * * *".to_string();

    let store = new_store();
    store.lock().unwrap().insert(
        "some-valid".into(),
        make_routine("some-valid", valid_title, agent_name),
    );
    store.lock().unwrap().insert("all-invalid".into(), fallback);

    let block = build_block(&store);

    assert!(block.contains("5 9 * * 1-5 "), "{block}");
    assert!(block.contains("15 10 * * * "), "{block}");
    assert!(!block.contains("not a cron"), "{block}");
    assert!(!block.contains("99 99 99 99 99"), "{block}");
    assert_eq!(
        std::fs::read_to_string(crate::paths::routine_compailed_cron_path(&valid_slug)).unwrap(),
        "5 9 * * 1-5\n"
    );
    assert_eq!(
        std::fs::read_to_string(crate::paths::routine_compailed_cron_path(&invalid_slug)).unwrap(),
        "15 10 * * *\n"
    );

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(valid_dir);
    let _ = std::fs::remove_dir_all(invalid_dir);
}

#[test]
fn build_block_keeps_valid_schedules_when_cron_union_cannot_simplify_them() {
    let agent_name = "test-sync-agent-cron-union-fallback-block";
    let title = "Cron Union Fallback Routine";
    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();
    let routine_dir = crate::paths::routine_dir(&slug);
    std::fs::create_dir_all(&routine_dir).unwrap();
    std::fs::write(routine_dir.join("schedule.cron"), "0 18 * * 0-4\n").unwrap();

    let store = new_store();
    store.lock().unwrap().insert(
        "cron-union-fallback".into(),
        make_routine("cron-union-fallback", title, agent_name),
    );

    let block = build_block(&store);

    assert!(block.contains("0 18 * * 0-4 "), "{block}");
    assert_eq!(
        std::fs::read_to_string(crate::paths::routine_compailed_cron_path(&slug)).unwrap(),
        "0 18 * * 0-4\n"
    );

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(routine_dir);
}

#[test]
fn build_block_falls_back_for_croner_valid_schedules_unsupported_by_cron_union() {
    let agent_name = "test-sync-agent-croner-only-fallback-block";
    let title = "Croner Only Fallback Routine";
    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();
    let routine_dir = crate::paths::routine_dir(&slug);
    std::fs::create_dir_all(&routine_dir).unwrap();
    std::fs::write(routine_dir.join("schedule.cron"), "0 18 L * *\n").unwrap();

    let store = new_store();
    store.lock().unwrap().insert(
        "croner-only-fallback".into(),
        make_routine("croner-only-fallback", title, agent_name),
    );

    let block = build_block(&store);

    assert!(block.contains("0 18 L * * "), "{block}");
    assert_eq!(
        std::fs::read_to_string(crate::paths::routine_compailed_cron_path(&slug)).unwrap(),
        "0 18 L * *\n"
    );

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(routine_dir);
}

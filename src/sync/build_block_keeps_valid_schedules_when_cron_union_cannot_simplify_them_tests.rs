
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

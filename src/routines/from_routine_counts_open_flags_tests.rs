
#[test]
fn from_routine_counts_open_flags() {
    let routine = Routine {
        id: "rid2".into(),
        schedule: "@daily".into(),
        schedules: vec![],
        title: "Flag Count Model Test ZZZ".into(),
        agent: "claude".into(),
        model: None,
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        disabled_reason: None,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
    };
    let slug = slugify(&routine.title);
    crate::routines::flags::create_flag(
        &slug,
        "bug",
        "d1",
        crate::routines::flags::FlagScope::General,
    )
    .unwrap();
    crate::routines::flags::create_flag(
        &slug,
        "gap",
        "d2",
        crate::routines::flags::FlagScope::Local,
    )
    .unwrap();

    let resp = RoutineResponse::from_routine(routine);
    assert_eq!(resp.flag_count, 2);

    crate::routine_storage::remove_routine_dir(&slug).unwrap();
}

#[test]
fn from_routine_agent_registered_false_for_malformed_config() {
    // Regression for #301: a present-but-malformed config is dropped at crontab-sync time, so it
    // must not report as registered — file existence alone is not enough. (The parseable and
    // absent cases are already covered by `from_routine_agent_command_available_true_when_command_resolves`
    // and `from_routine_agent_command_available_false_when_agent_not_registered` above.)
    let _home = TempHome::set();
    std::fs::create_dir_all(agent_toml_path("model-test-malformed").parent().unwrap()).unwrap();
    std::fs::write(agent_toml_path("model-test-malformed"), "command = [\n").unwrap();

    let resp = RoutineResponse::from_routine(make_routine("model-test-malformed"));
    assert!(!resp.agent_registered);
    assert!(!resp.agent_command_available);
}

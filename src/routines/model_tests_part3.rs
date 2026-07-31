
#[test]
fn from_routine_agent_setup_available_true_when_setup_interpreter_resolves() {
    // The `setup` step's first token (its interpreter/binary) is on `PATH` -> available, mirroring
    // the built-in `claude` agent's `python3 -c '...'` shape.
    let _home = TempHome::set();
    let dir = std::env::temp_dir().join(format!("moadim-model-setup-bin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("fake-interpreter");
    std::fs::write(&bin, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    std::fs::create_dir_all(
        agent_toml_path("model-test-setup-resolves")
            .parent()
            .unwrap(),
    )
    .unwrap();
    std::fs::write(
        agent_toml_path("model-test-setup-resolves"),
        "command = \"fake-agent-cmd\"\nsetup = \"fake-interpreter -c 'print(1)' \\\"$WB\\\"\"\n",
    )
    .unwrap();

    with_path(&dir, || {
        let resp = RoutineResponse::from_routine(make_routine("model-test-setup-resolves"));
        assert!(resp.agent_registered);
        assert!(resp.agent_setup_available);
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn from_routine_agent_setup_available_false_when_setup_interpreter_missing() {
    // A registered agent whose `command` may well resolve, but whose `setup` step shells out to an
    // interpreter that is not on `PATH` (e.g. `python3` on a python-less host) must report
    // `agent_setup_available: false` — this is issue #404's silent-no-op case: the launch command's
    // fail-fast guard aborts in `setup` before the agent ever starts.
    let _home = TempHome::set();
    std::fs::create_dir_all(
        agent_toml_path("model-test-setup-missing")
            .parent()
            .unwrap(),
    )
    .unwrap();
    std::fs::write(
        agent_toml_path("model-test-setup-missing"),
        "command = \"fake-agent-cmd\"\nsetup = \"definitely-not-a-real-binary-xyz -c 'print(1)'\"\n",
    )
    .unwrap();

    let empty_dir =
        std::env::temp_dir().join(format!("moadim-model-setup-empty-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&empty_dir).unwrap();

    with_path(&empty_dir, || {
        let resp = RoutineResponse::from_routine(make_routine("model-test-setup-missing"));
        assert!(resp.agent_registered);
        assert!(!resp.agent_setup_available);
    });

    let _ = std::fs::remove_dir_all(&empty_dir);
}

#[test]
fn from_routine_agent_setup_available_false_when_agent_not_registered() {
    // Mirrors `from_routine_agent_command_available_false_when_agent_not_registered`: no
    // `<agent>.toml` at all means there is no `setup` step to have resolved either.
    let _home = TempHome::set();
    let resp = RoutineResponse::from_routine(make_routine("model-test-setup-unregistered-zzz"));
    assert!(!resp.agent_registered);
    assert!(!resp.agent_setup_available);
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "model_tests_part2.rs"]
mod model_tests_part2;

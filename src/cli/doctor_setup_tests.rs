// Included from doctor_tests.rs, so it reuses that module's `Fixture`, `find` and `run_checks`.

#[test]
fn setup_interpreter_missing_fails_agent_check() {
    let fixture = Fixture::healthy();
    std::fs::remove_file(fixture.root.join("bin").join("python3")).unwrap();
    fixture.agent("claude", "command = \"claude\"\nsetup = \"python3 -c 'pass'\"\n");
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert_eq!(
        find(&checks, "agent claude").render(),
        "FAIL agent claude — setup step needs `python3`, not found on PATH"
    );
    assert_eq!(exit_code(&checks), 1);
}

#[test]
fn setup_interpreter_present_passes_agent_check() {
    let fixture = Fixture::healthy();
    fixture.agent("claude", "command = \"claude\"\nsetup = \"python3 -c 'pass'\"\n");
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert_eq!(find(&checks, "agent claude").render(), "ok   agent claude");
    assert_eq!(exit_code(&checks), 0);
}

#[test]
fn blank_setup_adds_no_extra_condition() {
    let fixture = Fixture::healthy();
    std::fs::remove_file(fixture.root.join("bin").join("python3")).unwrap();
    fixture.agent("claude", "command = \"claude\"\nsetup = \"  \"\n");
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert!(find(&checks, "agent claude").passed());
}

#[test]
fn command_missing_wins_over_setup_missing() {
    let fixture = Fixture::healthy();
    std::fs::remove_file(fixture.root.join("bin").join("python3")).unwrap();
    std::fs::remove_file(fixture.root.join("bin").join("claude")).unwrap();
    fixture.agent("claude", "command = \"claude\"\nsetup = \"python3 -c 'pass'\"\n");
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert_eq!(
        find(&checks, "agent claude").render(),
        "FAIL agent claude — `claude` not found on PATH"
    );
}

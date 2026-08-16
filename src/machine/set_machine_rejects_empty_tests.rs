#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn set_machine_rejects_empty() {
    let home = temp_home("set-empty");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    assert!(set_machine("   ").is_err());
    // Nothing was written.
    assert_eq!(read_machine_file(), None);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn set_machine_then_resolve_reads_file() {
    let home = temp_home("set-roundtrip");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _env = EnvGuard::unset("MOADIM_MACHINE");
    set_machine("  my-box  ").expect("write machine file");
    // Trimmed on write.
    assert_eq!(read_machine_file(), Some("my-box".to_string()));
    let (name, source) = resolve();
    assert_eq!(name, "my-box");
    assert_eq!(source, MachineSource::File);
    assert_eq!(current_machine(), "my-box");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn resolve_prefers_env_over_file() {
    let home = temp_home("env-over-file");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    set_machine("file-box").expect("write machine file");
    let _env = EnvGuard::set("MOADIM_MACHINE", "env-box");
    let (name, source) = resolve();
    assert_eq!(name, "env-box");
    assert_eq!(source, MachineSource::Env);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn referenced_machines_unions_routines() {
    let home = temp_home("referenced");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());

    let routine = crate::routines::Routine {
        id: "r1".to_string(),
        schedule: "0 9 * * *".to_string(),
        schedules: vec![],
        title: "Routine One".to_string(),
        agent: "claude".to_string(),
        model: None,
        prompt: "do".to_string(),
        goal: None,
        repositories: Vec::new(),
        machines: vec!["laptop".to_string(), "server".to_string()],
        tags: vec![],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
    };
    crate::routine_storage::write_routine(&routine).expect("write routine");

    let names = referenced_machines();
    let expected: std::collections::BTreeSet<String> = ["laptop", "server"]
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(names, expected);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn run_show_default_and_explicit() {
    let home = temp_home("run-show");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _env = EnvGuard::set("MOADIM_MACHINE", "showbox");
    assert_eq!(run(&[]), 0);
    assert_eq!(run(&["show".to_string()]), 0);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn run_set_writes_and_lists() {
    let home = temp_home("run-set");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _env = EnvGuard::unset("MOADIM_MACHINE");
    assert_eq!(run(&["set".to_string(), "boxy".to_string()]), 0);
    assert_eq!(read_machine_file(), Some("boxy".to_string()));
    // `list` with nothing referenced.
    assert_eq!(run(&["list".to_string()]), 0);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn run_set_without_name_is_usage_error() {
    assert_eq!(run(&["set".to_string()]), 2);
}

#[test]
fn run_unknown_subcommand_is_usage_error() {
    assert_eq!(run(&["bogus".to_string()]), 2);
}

#[test]
fn run_list_with_referenced_machine() {
    let home = temp_home("run-list");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let routine = crate::routines::Routine {
        id: "r1".to_string(),
        schedule: "0 9 * * *".to_string(),
        schedules: vec![],
        title: "Routine".to_string(),
        agent: "claude".to_string(),
        model: None,
        prompt: "do".to_string(),
        goal: None,
        repositories: Vec::new(),
        machines: vec!["alpha".to_string()],
        tags: vec![],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
    };
    crate::routine_storage::write_routine(&routine).expect("write routine");
    assert_eq!(run(&["list".to_string()]), 0);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn cmd_set_error_returns_one() {
    // An empty name makes `set_machine` fail, exercising the error branch (exit code 1).
    assert_eq!(cmd_set("   "), 1);
}

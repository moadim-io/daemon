//! Tests for machine identity resolution, persistence, CLI, and the targeting predicate.

use super::*;

/// Save an env var's prior value and restore it on drop, so a test's override never leaks. Tests in
/// this crate run single-threaded per binary (`RUST_TEST_THREADS=1`), so the global mutation is safe.
struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    /// Set `name` to `value`, remembering the prior value for restoration.
    fn set(name: &'static str, value: &str) -> EnvGuard {
        let previous = std::env::var_os(name);
        // SAFETY: single-threaded test execution.
        unsafe { std::env::set_var(name, value) }
        EnvGuard { name, previous }
    }

    /// Ensure `name` is unset for the duration of the guard.
    fn unset(name: &'static str) -> EnvGuard {
        let previous = std::env::var_os(name);
        // SAFETY: single-threaded test execution.
        unsafe { std::env::remove_var(name) }
        EnvGuard { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

/// Create a unique tempdir to use as `MOADIM_HOME_OVERRIDE` for a test.
fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-machine-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

// ─── resolve_from precedence ───────────────────────────────────────────────

#[test]
fn resolve_from_prefers_env() {
    let (name, source) = resolve_from(
        Some("from-env".to_string()),
        Some("from-file".to_string()),
        "from-host".to_string(),
    );
    assert_eq!(name, "from-env");
    assert_eq!(source, MachineSource::Env);
}

#[test]
fn resolve_from_uses_file_when_no_env() {
    let (name, source) = resolve_from(None, Some("from-file".to_string()), "from-host".to_string());
    assert_eq!(name, "from-file");
    assert_eq!(source, MachineSource::File);
}

#[test]
fn resolve_from_falls_back_to_hostname() {
    let (name, source) = resolve_from(None, None, "from-host".to_string());
    assert_eq!(name, "from-host");
    assert_eq!(source, MachineSource::Hostname);
}

#[test]
fn resolve_from_treats_blank_env_and_file_as_absent() {
    // Whitespace-only env and file values must not win — they fall through to the hostname.
    let (name, source) = resolve_from(
        Some("   ".to_string()),
        Some("\t\n".to_string()),
        "from-host".to_string(),
    );
    assert_eq!(name, "from-host");
    assert_eq!(source, MachineSource::Hostname);
}

#[test]
fn resolve_from_trims_winning_value() {
    let (name, source) = resolve_from(Some("  padded  ".to_string()), None, "host".to_string());
    assert_eq!(name, "padded");
    assert_eq!(source, MachineSource::Env);
}

// ─── non_empty ─────────────────────────────────────────────────────────────

#[test]
fn non_empty_filters_blank_and_none() {
    assert_eq!(non_empty(None), None);
    assert_eq!(non_empty(Some("   ".to_string())), None);
    assert_eq!(non_empty(Some(" ok ".to_string())), Some("ok".to_string()));
}

// ─── hostname ──────────────────────────────────────────────────────────────

#[test]
fn hostname_is_non_empty() {
    assert!(!hostname().is_empty());
}

// ─── targets predicate ─────────────────────────────────────────────────────

#[test]
fn targets_matches_only_named_machine() {
    assert!(targets(&["a".to_string(), "b".to_string()], "b"));
    assert!(!targets(&["a".to_string()], "b"));
    // Empty list targets no machine.
    assert!(!targets(&[], "a"));
}

// ─── MachineSource labels ──────────────────────────────────────────────────

#[test]
fn source_labels_are_distinct() {
    assert_eq!(MachineSource::Env.label(), "MOADIM_MACHINE env");
    assert_eq!(MachineSource::File.label(), "machine.local.toml");
    assert_eq!(MachineSource::Hostname.label(), "system hostname");
}

// ─── file persistence + end-to-end resolution ──────────────────────────────

#[test]
fn read_machine_file_absent_is_none() {
    let home = temp_home("read-absent");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    assert_eq!(read_machine_file(), None);
    let _ = std::fs::remove_dir_all(&home);
}

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

// ─── referenced_machines ───────────────────────────────────────────────────

#[test]
fn referenced_machines_unions_routines_and_jobs() {
    let home = temp_home("referenced");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());

    let routine = crate::routines::Routine {
        id: "r1".to_string(),
        schedule: "0 9 * * *".to_string(),
        title: "Routine One".to_string(),
        agent: "claude".to_string(),
        prompt: "do".to_string(),
        repositories: Vec::new(),
        machines: vec!["laptop".to_string(), "server".to_string()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: None,
        max_runtime_secs: None,
    };
    crate::routine_storage::write_routine(&routine).expect("write routine");

    let job = crate::cron_jobs::CronJob {
        id: "j1".to_string(),
        schedule: "0 9 * * *".to_string(),
        handler: "h".to_string(),
        metadata: serde_json::json!({}),
        machines: vec!["server".to_string(), "work".to_string()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
    };
    crate::storage::write_job(&job).expect("write job");

    let names = referenced_machines();
    let expected: std::collections::BTreeSet<String> = ["laptop", "server", "work"]
        .iter()
        .map(|name| name.to_string())
        .collect();
    assert_eq!(names, expected);
    let _ = std::fs::remove_dir_all(&home);
}

// ─── CLI dispatch (run) ────────────────────────────────────────────────────

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
        title: "Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "do".to_string(),
        repositories: Vec::new(),
        machines: vec!["alpha".to_string()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: None,
        max_runtime_secs: None,
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

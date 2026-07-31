#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::paths::agent_toml_path;

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing
/// the env var and the temp dir on drop. Keeps agent-registry reads (`agents_dir`/`agent_toml_path`)
/// off the developer's real `~/.config/moadim`. Tests in this crate run single-threaded
/// (`RUST_TEST_THREADS=1`), so the global env mutation is safe. Mirrors the identical helper in
/// `service_tests.rs`.
struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-modeltest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run `body` with `PATH` set to `value`, restoring the previous value afterwards. Mirrors the
/// identical helper in `command_tests.rs`.
fn with_path(value: &std::path::Path, body: impl FnOnce()) {
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; the value is restored immediately after.
    unsafe {
        std::env::set_var("PATH", value);
    }
    body();
    // SAFETY: single-threaded test execution.
    unsafe {
        match saved {
            Some(prev) => std::env::set_var("PATH", prev),
            None => std::env::remove_var("PATH"),
        }
    }
}

/// Build a minimal routine referencing `agent`.
fn make_routine(agent: &str) -> Routine {
    Routine {
        id: "model-test-id".into(),
        schedule: "@daily".into(),
        schedules: vec![],
        title: "Model Test Routine".into(),
        agent: agent.into(),
        model: None,
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".into(),
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
fn from_routine_agent_command_available_true_when_command_resolves() {
    let _home = TempHome::set();
    let dir = std::env::temp_dir().join(format!("moadim-model-bin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("fake-agent-cmd");
    std::fs::write(&bin, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    std::fs::create_dir_all(agent_toml_path("model-test-resolves").parent().unwrap()).unwrap();
    std::fs::write(
        agent_toml_path("model-test-resolves"),
        r#"command = "fake-agent-cmd""#,
    )
    .unwrap();

    with_path(&dir, || {
        let resp = RoutineResponse::from_routine(make_routine("model-test-resolves"));
        assert!(resp.agent_registered);
        assert!(resp.agent_command_available);
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn from_routine_agent_command_available_false_when_registered_but_not_on_path() {
    // A present, well-formed agent config whose `command` is not installed must report
    // `agent_command_available: false` while `agent_registered` stays `true` — the two are
    // distinct signals (see the field's doc comment).
    let _home = TempHome::set();
    std::fs::create_dir_all(agent_toml_path("model-test-unresolved").parent().unwrap()).unwrap();
    std::fs::write(
        agent_toml_path("model-test-unresolved"),
        r#"command = "definitely-not-a-real-binary-xyz""#,
    )
    .unwrap();

    let empty_dir =
        std::env::temp_dir().join(format!("moadim-model-empty-bin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&empty_dir).unwrap();

    with_path(&empty_dir, || {
        let resp = RoutineResponse::from_routine(make_routine("model-test-unresolved"));
        assert!(resp.agent_registered);
        assert!(!resp.agent_command_available);
    });

    let _ = std::fs::remove_dir_all(&empty_dir);
}

#[test]
fn from_routine_agent_command_available_false_when_agent_not_registered() {
    // No `<agent>.toml` at all: `load_agent_command` errors, so `agent_command_available` falls
    // back to `false` via `unwrap_or(false)` alongside `agent_registered: false`.
    let _home = TempHome::set();
    let resp = RoutineResponse::from_routine(make_routine("model-test-unregistered-zzz"));
    assert!(!resp.agent_registered);
    assert!(!resp.agent_command_available);
}

#[test]
fn from_routine_agent_setup_available_true_when_no_setup_step() {
    // An agent config with no `setup` step at all has nothing to probe, so it reports available —
    // the vacuous-true arm of `setup_step_available`.
    let _home = TempHome::set();
    std::fs::create_dir_all(agent_toml_path("model-test-no-setup").parent().unwrap()).unwrap();
    std::fs::write(
        agent_toml_path("model-test-no-setup"),
        r#"command = "fake-agent-cmd""#,
    )
    .unwrap();

    let resp = RoutineResponse::from_routine(make_routine("model-test-no-setup"));
    assert!(resp.agent_registered);
    assert!(resp.agent_setup_available);
}

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

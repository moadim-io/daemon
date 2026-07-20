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
fn describe_schedule_appends_timezone_when_present() {
    let desc = describe_schedule("@daily", Some("Asia/Jerusalem")).unwrap();
    assert!(
        desc.ends_with("(Asia/Jerusalem)"),
        "expected timezone suffix in {desc}"
    );
}

#[test]
fn describe_schedule_omits_timezone_when_none() {
    // The `None` arm returns the bare description with no parenthesized timezone.
    let desc = describe_schedule("@daily", None).unwrap();
    assert!(!desc.contains('('), "expected no timezone suffix in {desc}");
}

#[test]
fn describe_schedule_returns_none_for_unparseable() {
    assert!(describe_schedule("@reboot", Some("UTC")).is_none());
    assert!(describe_schedule("not a cron", None).is_none());
}

#[test]
fn next_run_at_some_for_enabled_parseable_schedule() {
    assert!(next_run_at("@daily", true).is_some());
}

#[test]
fn next_run_at_uses_cron_union_for_standard_crons() {
    assert!(next_run_at("*/5 * * * *", true).is_some());
}

#[test]
fn next_run_at_none_when_disabled() {
    assert!(next_run_at("@daily", false).is_none());
}

#[test]
fn next_run_at_none_for_unparseable_schedule() {
    assert!(next_run_at("@reboot", true).is_none());
    assert!(next_run_at("not a cron", true).is_none());
}

#[test]
fn next_run_at_none_for_a_schedule_with_no_future_fire() {
    // A parseable 7-field (sec min hour dom month dow year) schedule pinned to a year that has
    // already passed matches croner's own syntax, so `.parse()` succeeds, but `iter_after(now)`
    // never yields an occurrence — covering the third documented `None` case (no upcoming fire)
    // distinct from "unparseable" and "disabled".
    assert!(next_run_at("0 0 0 1 1 * 2020", true).is_none());
}

#[test]
fn from_routine_populates_derived_fields() {
    let routine = Routine {
        id: "rid".into(),
        schedule: "@daily".into(),
        title: "My Title".into(),
        agent: "claude".into(),
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
    };
    let resp = RoutineResponse::from_routine(routine);
    assert!(resp.schedule_description.is_some());
    assert!(resp.file_path.contains("routine.toml"));
    assert_eq!(resp.flag_count, 0);
    assert!(resp.next_run_at.is_some());
    // No `MOADIM_TMUX_BIN` stub set: the test-build fallback tmux binary doesn't exist, so no
    // session can be reported alive.
    assert!(!resp.is_running);
}

#[test]
fn from_routine_is_running_true_when_a_fire_has_a_live_tmux_session() {
    // Mirrors `svc_trigger_skips_spawn_when_a_previous_run_is_still_alive`
    // (service_overlap_guard_tests.rs): a tmux stub that reports a session under this routine's
    // `moadim-{slug}-` prefix must surface as `is_running: true`, the same overlap-guard probe
    // `svc_trigger` uses (#514), now exposed on the read path too (#438).
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let title = "Model Test Is Running ZZZ";
    let slug = slugify(title);
    let dir = std::env::temp_dir().join(format!("moadim-model-running-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("tmux");
    std::fs::write(
        &stub,
        format!("#!/bin/sh\nprintf 'moadim-{slug}-1730000000_4821\\n'\nexit 0\n"),
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut routine = make_routine("claude");
    routine.title = title.into();

    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
    unsafe { std::env::set_var("MOADIM_TMUX_BIN", &stub) };

    let resp = RoutineResponse::from_routine(routine);

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);

    assert!(resp.is_running);
}

#[test]
fn from_routine_counts_open_flags() {
    let routine = Routine {
        id: "rid2".into(),
        schedule: "@daily".into(),
        title: "Flag Count Model Test ZZZ".into(),
        agent: "claude".into(),
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

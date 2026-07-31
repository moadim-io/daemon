#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
    assert!(next_run_at(&["@daily".to_string()], true).is_some());
}

#[test]
fn next_run_at_uses_cron_union_for_standard_crons() {
    assert!(next_run_at(&["*/5 * * * *".to_string()], true).is_some());
}

#[test]
fn next_run_at_none_when_disabled() {
    assert!(next_run_at(&["@daily".to_string()], false).is_none());
}

#[test]
fn next_run_at_none_for_unparseable_schedule() {
    assert!(next_run_at(&["@reboot".to_string()], true).is_none());
    assert!(next_run_at(&["@midnight".to_string()], true).is_none());
    assert!(next_run_at(&["not a cron".to_string()], true).is_none());
}

#[test]
fn next_run_at_none_for_impossible_calendar_date() {
    // Feb 30 never occurs, so a schedule pinned to it parses fine but has no upcoming fire —
    // covers `next_run_at`'s "no upcoming fire" branch, distinct from the unparseable-schedule
    // case above.
    assert!(next_run_at(&["0 0 30 2 *".to_string()], true).is_none());
}

#[test]
fn from_routine_populates_derived_fields() {
    let routine = Routine {
        id: "rid".into(),
        schedule: "@daily".into(),
        schedules: vec![],
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
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
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
include!("from_routine_counts_open_flags_tests.rs");

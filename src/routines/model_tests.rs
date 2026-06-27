#![allow(clippy::missing_docs_in_private_items)]

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
fn from_routine_populates_derived_fields() {
    let routine = Routine {
        id: "rid".into(),
        schedule: "@daily".into(),
        title: "My Title".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: None,
        max_runtime_secs: None,
    };
    let resp = RoutineResponse::from_routine(routine);
    assert!(resp.schedule_description.is_some());
    assert!(resp.file_path.contains("routine.toml"));
}

/// Build a minimal routine pointing at `agent`, for `agent_registered` derivation tests.
fn routine_with_agent(agent: &str) -> Routine {
    Routine {
        id: "rid".into(),
        schedule: "@daily".into(),
        title: "My Title".into(),
        agent: agent.into(),
        prompt: "p".into(),
        repositories: vec![],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

#[test]
fn from_routine_marks_agent_registered_for_a_parseable_config() {
    // A config that exists *and* parses is the only case that actually fires at sync time.
    let agent = "model-agent-registered-valid-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent);
    std::fs::write(&cfg, "command = \"claude\"\n").unwrap();

    let resp = RoutineResponse::from_routine(routine_with_agent(agent));
    assert!(resp.agent_registered);

    std::fs::remove_file(&cfg).unwrap();
}

#[test]
fn from_routine_marks_agent_unregistered_for_a_malformed_config() {
    // Regression for #301: a present-but-malformed config is dropped at crontab-sync time, so it
    // must NOT report as registered — file existence alone is not enough.
    let agent = "model-agent-registered-malformed-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent);
    std::fs::write(&cfg, "command = [\n").unwrap();

    let resp = RoutineResponse::from_routine(routine_with_agent(agent));
    assert!(!resp.agent_registered);

    std::fs::remove_file(&cfg).unwrap();
}

#[test]
fn from_routine_marks_agent_unregistered_when_config_absent() {
    // No config on disk → not registered, unchanged from the previous behavior.
    let resp =
        RoutineResponse::from_routine(routine_with_agent("model-agent-registered-absent-zzz"));
    assert!(!resp.agent_registered);
}

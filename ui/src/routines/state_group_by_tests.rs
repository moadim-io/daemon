use super::super::model::Repository;
use super::*;

/// Build a routine with the fields the filter/state helpers read; the rest are inert.
fn routine(
    id: &str,
    title: &str,
    agent: &str,
    schedule: &str,
    machines: &[&str],
    repos: &[&str],
    enabled: bool,
) -> Routine {
    Routine {
        id: id.into(),
        title: title.into(),
        agent: agent.into(),
        model: None,
        schedule: schedule.into(),
        prompt: String::new(),
        repositories: repos
            .iter()
            .map(|r| Repository {
                repository: (*r).to_string(),
                branch: None,
            })
            .collect(),
        machines: machines.iter().map(|m| (*m).to_string()).collect(),
        enabled,
        source: String::new(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        ttl_secs: None,
        tags: vec![],
        agent_registered: false,
        file_path: String::new(),
        schedule_description: None,
        goal: None,
        flag_count: 0,
    }
}

// ── RGroupBy codec ────────────────────────────────────────────────────────────

#[test]
fn r_group_by_as_str_roundtrips() {
    for by in [
        RGroupBy::None,
        RGroupBy::Agent,
        RGroupBy::Machine,
        RGroupBy::Status,
        RGroupBy::Health,
    ] {
        assert_eq!(RGroupBy::from_str(by.as_str()), by);
    }
}

#[test]
fn r_group_by_default_is_none() {
    assert_eq!(RGroupBy::default(), RGroupBy::None);
}

#[test]
fn r_group_by_unknown_token_decodes_to_none() {
    assert_eq!(RGroupBy::from_str("bogus"), RGroupBy::None);
    assert_eq!(RGroupBy::from_str(""), RGroupBy::None);
}

// ── routine_group_key ─────────────────────────────────────────────────────────

#[test]
fn routine_group_key_agent_returns_agent_field() {
    let r = routine("id1", "t", "claude", "0 * * * *", &["m1"], &[], true);
    assert_eq!(routine_group_key(&r, RGroupBy::Agent), "claude");
}

#[test]
fn routine_group_key_machine_returns_first_machine() {
    let r = routine(
        "id1",
        "t",
        "claude",
        "0 * * * *",
        &["alpha", "beta"],
        &[],
        true,
    );
    assert_eq!(routine_group_key(&r, RGroupBy::Machine), "alpha");
}

#[test]
fn routine_group_key_machine_returns_unassigned_when_no_machines() {
    let r = routine("id1", "t", "claude", "0 * * * *", &[], &[], true);
    assert_eq!(routine_group_key(&r, RGroupBy::Machine), "(unassigned)");
}

#[test]
fn routine_group_key_status_enabled() {
    let r = routine("id1", "t", "claude", "0 * * * *", &[], &[], true);
    assert_eq!(routine_group_key(&r, RGroupBy::Status), "Enabled");
}

#[test]
fn routine_group_key_status_disabled() {
    let r = routine("id1", "t", "claude", "0 * * * *", &[], &[], false);
    assert_eq!(routine_group_key(&r, RGroupBy::Status), "Disabled");
}

#[test]
fn routine_group_key_none_returns_empty_string() {
    let r = routine("id1", "t", "claude", "0 * * * *", &[], &[], true);
    assert_eq!(routine_group_key(&r, RGroupBy::None), "");
}

// ── group_routines ────────────────────────────────────────────────────────────

#[test]
fn group_routines_none_returns_single_group_with_all_routines() {
    let rs = vec![
        routine("a", "t", "claude", "0 * * * *", &[], &[], true),
        routine("b", "t", "codex", "0 * * * *", &[], &[], false),
    ];
    let groups = group_routines(&rs, RGroupBy::None);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, "");
    assert_eq!(groups[0].1.len(), 2);
}

#[test]
fn group_routines_by_agent_creates_one_group_per_agent() {
    let rs = vec![
        routine("a", "t", "claude", "0 * * * *", &[], &[], true),
        routine("b", "t", "codex", "0 * * * *", &[], &[], true),
        routine("c", "t", "claude", "0 * * * *", &[], &[], true),
    ];
    let groups = group_routines(&rs, RGroupBy::Agent);
    // BTreeMap → alphabetical: claude, codex
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "claude");
    assert_eq!(groups[0].1.len(), 2);
    assert_eq!(groups[1].0, "codex");
    assert_eq!(groups[1].1.len(), 1);
}

#[test]
fn group_routines_by_agent_preserves_input_order_within_group() {
    let rs = vec![
        routine("first", "t", "claude", "0 * * * *", &[], &[], true),
        routine("second", "t", "claude", "0 * * * *", &[], &[], true),
    ];
    let groups = group_routines(&rs, RGroupBy::Agent);
    assert_eq!(groups[0].1[0].id, "first");
    assert_eq!(groups[0].1[1].id, "second");
}

#[test]
fn group_routines_by_machine_separates_unassigned() {
    let rs = vec![
        routine("a", "t", "claude", "0 * * * *", &["worker-1"], &[], true),
        routine("b", "t", "claude", "0 * * * *", &[], &[], true),
        routine("c", "t", "claude", "0 * * * *", &["worker-1"], &[], true),
    ];
    let groups = group_routines(&rs, RGroupBy::Machine);
    // alphabetical: "(unassigned)" sorts before "worker-1"
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "(unassigned)");
    assert_eq!(groups[0].1.len(), 1);
    assert_eq!(groups[1].0, "worker-1");
    assert_eq!(groups[1].1.len(), 2);
}

#[test]
fn group_routines_by_status_splits_enabled_and_disabled() {
    let rs = vec![
        routine("a", "t", "claude", "0 * * * *", &[], &[], true),
        routine("b", "t", "claude", "0 * * * *", &[], &[], false),
        routine("c", "t", "claude", "0 * * * *", &[], &[], true),
    ];
    let groups = group_routines(&rs, RGroupBy::Status);
    // alphabetical: "Disabled" before "Enabled"
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "Disabled");
    assert_eq!(groups[0].1.len(), 1);
    assert_eq!(groups[1].0, "Enabled");
    assert_eq!(groups[1].1.len(), 2);
}

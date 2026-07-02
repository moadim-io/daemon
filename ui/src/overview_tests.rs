//! Host-side unit tests for the overview page's pure aggregation logic: KPI
//! counting, the merged soonest-first upcoming list, the next-run summary, and
//! the record→`SchedSource` mappers. All deterministic given a fixed `now`.

use super::*;
use chrono::{Local, TimeZone};

/// A fixed reference instant (10:00 local) so cron math is deterministic.
fn at_ten() -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 6, 22, 10, 0, 0)
        .single()
        .expect("valid local time")
}

/// A healthy source: targets a machine and a registered agent.
/// Attention tests opt into a fault by mutating the returned value.
fn src(kind: Kind, label: &str, schedule: &str, enabled: bool) -> SchedSource {
    SchedSource {
        kind,
        id: label.into(),
        label: label.into(),
        schedule: schedule.into(),
        human: None,
        enabled,
        machines_empty: false,
        agent_registered: match kind {
            Kind::Routine => Some(true),
        },
    }
}

#[test]
fn kpis_count_total_enabled_disabled_due_soon() {
    let sources = vec![
        src(Kind::Routine, "a", "*/5 * * * *", true), // enabled, fires in 5m → due soon
        src(Kind::Routine, "b", "0 0 * * *", true),   // enabled, fires at midnight → far
        src(Kind::Routine, "c", "*/5 * * * *", false), // disabled → never due
    ];
    let kpis = compute_kpis(&sources, at_ten());
    assert_eq!(kpis.total, 3);
    assert_eq!(kpis.enabled, 2);
    assert_eq!(kpis.disabled, 1);
    assert_eq!(kpis.due_soon, 1);
}

#[test]
fn kpis_default_is_zeroed() {
    let kpis = Kpis::default();
    assert_eq!(kpis.total, 0);
    assert_eq!(kpis.enabled, 0);
    assert_eq!(kpis.disabled, 0);
    assert_eq!(kpis.due_soon, 0);
}

#[test]
fn upcoming_sorted_soonest_first_excludes_disabled_and_invalid() {
    let sources = vec![
        src(Kind::Routine, "midnight", "0 0 * * *", true),
        src(Kind::Routine, "five", "*/5 * * * *", true),
        src(Kind::Routine, "off", "*/1 * * * *", false), // disabled → excluded
        src(Kind::Routine, "bad", "not a cron", true),   // invalid → excluded
    ];
    let runs = upcoming_runs(&sources, at_ten());
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].label, "five"); // 10:05 sorts before midnight
    assert_eq!(runs[0].kind, Kind::Routine);
    assert_eq!(runs[1].label, "midnight");
    assert!(runs[0].soon);
    assert!(!runs[1].soon);
}

#[test]
fn upcoming_truncates_to_limit() {
    let sources: Vec<SchedSource> = (0..UPCOMING_LIMIT + 4)
        .map(|i| src(Kind::Routine, &format!("job{i:02}"), "*/5 * * * *", true))
        .collect();
    let runs = upcoming_runs(&sources, at_ten());
    assert_eq!(runs.len(), UPCOMING_LIMIT);
}

#[test]
fn upcoming_ties_break_by_label() {
    let sources = vec![
        src(Kind::Routine, "zeta", "*/5 * * * *", true),
        src(Kind::Routine, "alpha", "*/5 * * * *", true),
    ];
    let runs = upcoming_runs(&sources, at_ten());
    assert_eq!(runs[0].label, "alpha");
    assert_eq!(runs[1].label, "zeta");
}

#[test]
fn upcoming_preserves_human_description() {
    let mut source = src(Kind::Routine, "nightly", "0 0 * * *", true);
    source.human = Some("At midnight".into());
    let runs = upcoming_runs(&[source], at_ten());
    assert_eq!(runs[0].human.as_deref(), Some("At midnight"));
}

#[test]
fn next_run_summary_is_first_or_none() {
    let now = at_ten();
    assert_eq!(next_run_summary(&[], now), None);
    let runs = upcoming_runs(&[src(Kind::Routine, "five", "*/5 * * * *", true)], now);
    assert_eq!(next_run_summary(&runs, now), Some("in 5m".to_string()));
}

#[test]
fn from_routine_uses_title_as_label() {
    let routine: Routine = serde_json::from_value(serde_json::json!({
        "id": "r1",
        "schedule": "0 0 * * *",
        "title": "Nightly sweep",
        "agent": "claude",
        "prompt": "go",
        "enabled": false
    }))
    .expect("valid routine json");
    let source = from_routine(&routine);
    assert_eq!(source.kind, Kind::Routine);
    assert_eq!(source.id, "r1");
    assert_eq!(source.label, "Nightly sweep");
    assert_eq!(source.schedule, "0 0 * * *");
    assert!(!source.enabled);
}

// ── NEEDS ATTENTION triage ──────────────────────────────────────────────────

#[test]
fn attention_reason_skips_disabled_even_when_broken() {
    // A disabled entity is intentional, never flagged — even with every fault.
    let mut s = src(Kind::Routine, "off", "not a cron", false);
    s.machines_empty = true;
    s.agent_registered = Some(false);
    assert_eq!(attention_reason(&s, at_ten()), None);
}

#[test]
fn attention_reason_healthy_is_none() {
    let s = src(Kind::Routine, "ok", "*/5 * * * *", true);
    assert_eq!(attention_reason(&s, at_ten()), None);
}

#[test]
fn attention_reason_dormant_outranks_other_faults() {
    // No machine + dead schedule + missing agent → dormant wins (highest priority).
    let mut s = src(Kind::Routine, "r", "not a cron", true);
    s.machines_empty = true;
    s.agent_registered = Some(false);
    assert_eq!(
        attention_reason(&s, at_ten()),
        Some(AttentionReason::Dormant)
    );
}

#[test]
fn attention_reason_dead_schedule_when_no_future_fire() {
    // Has a machine, but the expression never parses → no future fire.
    let s = src(Kind::Routine, "c", "not a cron", true);
    assert_eq!(
        attention_reason(&s, at_ten()),
        Some(AttentionReason::DeadSchedule)
    );
}

#[test]
fn attention_reason_agent_missing_only_when_schedule_lives() {
    let mut s = src(Kind::Routine, "r", "*/5 * * * *", true);
    s.agent_registered = Some(false);
    assert_eq!(
        attention_reason(&s, at_ten()),
        Some(AttentionReason::AgentUnregistered)
    );
}

#[test]
fn attention_reason_no_agent_fault_when_registered() {
    // A routine with a registered agent never flags the agent fault.
    let s = src(Kind::Routine, "c", "*/5 * * * *", true);
    assert_eq!(attention_reason(&s, at_ten()), None);
}

#[test]
fn attention_items_sorted_by_rank_then_label() {
    let mut dead = src(Kind::Routine, "zeta-dead", "not a cron", true);
    dead.machines_empty = false;
    let mut dormant_z = src(Kind::Routine, "zeta-dormant", "*/5 * * * *", true);
    dormant_z.machines_empty = true;
    let mut dormant_a = src(Kind::Routine, "alpha-dormant", "*/5 * * * *", true);
    dormant_a.machines_empty = true;
    let mut agent = src(Kind::Routine, "agent-missing", "*/5 * * * *", true);
    agent.agent_registered = Some(false);
    let healthy = src(Kind::Routine, "fine", "*/5 * * * *", true);

    let items = attention_items(&[dead, dormant_z, dormant_a, agent, healthy], at_ten());
    // Healthy one excluded; dormant (rank 0) first, ties by label, then dead, then agent.
    assert_eq!(items.len(), 4);
    assert_eq!(items[0].reason, AttentionReason::Dormant);
    assert_eq!(items[0].label, "alpha-dormant");
    assert_eq!(items[1].reason, AttentionReason::Dormant);
    assert_eq!(items[1].label, "zeta-dormant");
    assert_eq!(items[2].reason, AttentionReason::DeadSchedule);
    assert_eq!(items[3].reason, AttentionReason::AgentUnregistered);
}

#[test]
fn attention_items_empty_for_healthy_fleet() {
    let items = attention_items(
        &[
            src(Kind::Routine, "a", "*/5 * * * *", true),
            src(Kind::Routine, "b", "0 0 * * *", true),
        ],
        at_ten(),
    );
    assert!(items.is_empty());
}

#[test]
fn kpis_count_attention() {
    let mut dormant = src(Kind::Routine, "d", "*/5 * * * *", true);
    dormant.machines_empty = true;
    let sources = vec![
        dormant,
        src(Kind::Routine, "ok", "*/5 * * * *", true),
        src(Kind::Routine, "off", "not a cron", false), // disabled → not counted
    ];
    let kpis = compute_kpis(&sources, at_ten());
    assert_eq!(kpis.attention, 1);
}

#[test]
fn kpis_default_attention_is_zero() {
    assert_eq!(Kpis::default().attention, 0);
}

#[test]
fn attention_reason_rank_badge_detail_cover_all_variants() {
    for r in [
        AttentionReason::Dormant,
        AttentionReason::DeadSchedule,
        AttentionReason::AgentUnregistered,
    ] {
        assert!(!r.badge().is_empty());
        assert!(!r.detail().is_empty());
    }
    assert!(AttentionReason::Dormant.rank() < AttentionReason::DeadSchedule.rank());
    assert!(AttentionReason::DeadSchedule.rank() < AttentionReason::AgentUnregistered.rank());
}

#[test]
fn from_routine_carries_agent_registration_and_machines() {
    let routine: Routine = serde_json::from_value(serde_json::json!({
        "id": "r1", "schedule": "0 0 * * *", "title": "T", "agent": "a", "prompt": "p",
        "machines": ["box-1"], "enabled": true, "agent_registered": false
    }))
    .expect("valid routine json");
    let s = from_routine(&routine);
    assert_eq!(s.agent_registered, Some(false));
    assert!(!s.machines_empty);
}

#[test]
fn upcoming_run_carries_id() {
    let source = src(Kind::Routine, "daily-backup", "*/5 * * * *", true);
    let runs = upcoming_runs(&[source], at_ten());
    assert_eq!(runs[0].id, "daily-backup");
    assert_eq!(runs[0].label, "daily-backup");
}

#[test]
fn upcoming_run_routine_id_differs_from_label() {
    let routine: Routine = serde_json::from_value(serde_json::json!({
        "id": "abc-uuid-123",
        "schedule": "*/5 * * * *",
        "title": "My Routine",
        "agent": "claude",
        "prompt": "do something",
        "enabled": true
    }))
    .expect("valid routine json");
    let source = from_routine(&routine);
    assert_eq!(source.id, "abc-uuid-123");
    assert_eq!(source.label, "My Routine");
    let runs = upcoming_runs(&[source], at_ten());
    assert_eq!(runs[0].id, "abc-uuid-123");
    assert_eq!(runs[0].label, "My Routine");
}

#[test]
fn sources_of_maps_routines() {
    let routine: Routine = serde_json::from_value(serde_json::json!({
        "id": "r1", "schedule": "0 0 * * *", "title": "T", "agent": "a",
        "prompt": "p", "enabled": true
    }))
    .expect("valid routine json");
    let sources = sources_of(&[routine]);
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].kind, Kind::Routine);
    assert_eq!(sources[0].label, "T");
}

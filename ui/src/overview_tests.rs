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

fn src(kind: Kind, label: &str, schedule: &str, enabled: bool) -> SchedSource {
    SchedSource {
        kind,
        label: label.into(),
        schedule: schedule.into(),
        human: None,
        enabled,
    }
}

#[test]
fn kpis_count_total_enabled_disabled_due_soon() {
    let sources = vec![
        src(Kind::Cron, "a", "*/5 * * * *", true), // enabled, fires in 5m → due soon
        src(Kind::Routine, "b", "0 0 * * *", true), // enabled, fires at midnight → far
        src(Kind::Cron, "c", "*/5 * * * *", false), // disabled → never due
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
        src(Kind::Cron, "five", "*/5 * * * *", true),
        src(Kind::Cron, "off", "*/1 * * * *", false), // disabled → excluded
        src(Kind::Cron, "bad", "not a cron", true),   // invalid → excluded
    ];
    let runs = upcoming_runs(&sources, at_ten());
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].label, "five"); // 10:05 sorts before midnight
    assert_eq!(runs[0].kind, Kind::Cron);
    assert_eq!(runs[1].label, "midnight");
    assert!(runs[0].soon);
    assert!(!runs[1].soon);
}

#[test]
fn upcoming_truncates_to_limit() {
    let sources: Vec<SchedSource> = (0..UPCOMING_LIMIT + 4)
        .map(|i| src(Kind::Cron, &format!("job{i:02}"), "*/5 * * * *", true))
        .collect();
    let runs = upcoming_runs(&sources, at_ten());
    assert_eq!(runs.len(), UPCOMING_LIMIT);
}

#[test]
fn upcoming_ties_break_by_label() {
    let sources = vec![
        src(Kind::Cron, "zeta", "*/5 * * * *", true),
        src(Kind::Cron, "alpha", "*/5 * * * *", true),
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
    let runs = upcoming_runs(&[src(Kind::Cron, "five", "*/5 * * * *", true)], now);
    assert_eq!(next_run_summary(&runs, now), Some("in 5m".to_string()));
}

#[test]
fn from_cron_maps_label_and_schedule() {
    let job: CronJob = serde_json::from_value(serde_json::json!({
        "id": "backup",
        "schedule": "*/5 * * * *",
        "handler": "h",
        "metadata": {},
        "enabled": true,
        "created_at": 0,
        "updated_at": 0,
        "schedule_description": "Every 5 minutes"
    }))
    .expect("valid cron job json");
    let source = from_cron(&job);
    assert_eq!(source.kind, Kind::Cron);
    assert_eq!(source.label, "backup");
    assert_eq!(source.schedule, "*/5 * * * *");
    assert_eq!(source.human.as_deref(), Some("Every 5 minutes"));
    assert!(source.enabled);
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
    assert_eq!(source.label, "Nightly sweep");
    assert_eq!(source.schedule, "0 0 * * *");
    assert!(!source.enabled);
}

#[test]
fn sources_of_concatenates_crons_then_routines() {
    let job: CronJob = serde_json::from_value(serde_json::json!({
        "id": "c1", "schedule": "*/5 * * * *", "handler": "h", "metadata": {},
        "enabled": true, "created_at": 0, "updated_at": 0
    }))
    .expect("valid cron job json");
    let routine: Routine = serde_json::from_value(serde_json::json!({
        "id": "r1", "schedule": "0 0 * * *", "title": "T", "agent": "a",
        "prompt": "p", "enabled": true
    }))
    .expect("valid routine json");
    let sources = sources_of(&[job], &[routine]);
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].kind, Kind::Cron);
    assert_eq!(sources[1].kind, Kind::Routine);
    assert_eq!(sources[1].label, "T");
}

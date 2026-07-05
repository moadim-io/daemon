use super::super::model::Repository;
use super::*;
use chrono::TimeZone;

/// Build a routine with the fields the filter reads; the rest are inert.
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
        ttl_secs: None,
        tags: vec![],
        agent_registered: false,
        file_path: String::new(),
        schedule_description: None,
        goal: None,
        flag_count: 0,
    }
}

/// Fixed deterministic "now" for tests (2026-01-01 12:00:00 local).
fn now() -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
}

// ── last_fire_at ──────────────────────────────────────────────────────────────

fn routine_with_triggers(last_manual: Option<u64>, last_scheduled: Option<u64>) -> Routine {
    Routine {
        last_manual_trigger_at: last_manual,
        last_scheduled_trigger_at: last_scheduled,
        ..routine("id", "My Routine", "claude", "0 * * * *", &[], &[], true)
    }
}

#[test]
fn last_fire_at_none_when_never_triggered() {
    let r = routine_with_triggers(None, None);
    assert_eq!(last_fire_at(&r), None);
}

#[test]
fn last_fire_at_manual_only() {
    let r = routine_with_triggers(Some(100), None);
    assert_eq!(last_fire_at(&r), Some(100));
}

#[test]
fn last_fire_at_scheduled_only() {
    let r = routine_with_triggers(None, Some(200));
    assert_eq!(last_fire_at(&r), Some(200));
}

#[test]
fn last_fire_at_returns_max_when_manual_is_later() {
    let r = routine_with_triggers(Some(300), Some(100));
    assert_eq!(last_fire_at(&r), Some(300));
}

#[test]
fn last_fire_at_returns_max_when_scheduled_is_later() {
    let r = routine_with_triggers(Some(100), Some(300));
    assert_eq!(last_fire_at(&r), Some(300));
}

#[test]
fn last_fire_at_equal_timestamps_returns_that_value() {
    let r = routine_with_triggers(Some(500), Some(500));
    assert_eq!(last_fire_at(&r), Some(500));
}

// ── routine_health ────────────────────────────────────────────────────────────

#[test]
fn health_disabled_routine_is_disabled() {
    let r = routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], false);
    assert_eq!(routine_health(&r, now()), RoutineHealth::Disabled);
}

#[test]
fn health_enabled_no_machines_is_dormant() {
    let r = Routine {
        agent_registered: true,
        ..routine("a", "A", "claude", "0 * * * *", &[], &[], true)
    };
    assert_eq!(routine_health(&r, now()), RoutineHealth::Dormant);
}

#[test]
fn health_enabled_blank_machine_entry_is_dormant() {
    let r = Routine {
        agent_registered: true,
        ..routine("a", "A", "claude", "0 * * * *", &["   "], &[], true)
    };
    assert_eq!(routine_health(&r, now()), RoutineHealth::Dormant);
}

#[test]
fn health_dead_schedule_is_dead() {
    let r = Routine {
        agent_registered: true,
        ..routine(
            "a",
            "A",
            "claude",
            "not-a-valid-cron",
            &["machine1"],
            &[],
            true,
        )
    };
    assert_eq!(routine_health(&r, now()), RoutineHealth::DeadSchedule);
}

#[test]
fn health_missing_agent_is_agent_missing() {
    // agent_registered defaults to false in routine()
    let r = routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true);
    assert_eq!(routine_health(&r, now()), RoutineHealth::AgentMissing);
}

#[test]
fn health_fully_configured_is_healthy() {
    let r = Routine {
        agent_registered: true,
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert_eq!(routine_health(&r, now()), RoutineHealth::Healthy);
}

// ─── snooze_detail ────────────────────────────────────────────────────────────

#[test]
fn snooze_detail_empty_when_not_snoozed() {
    let r = routine("a", "A", "claude", "0 * * * *", &["m"], &[], true);
    assert_eq!(snooze_detail(&r, now()), "");
}

#[test]
fn snooze_detail_shows_minutes_left_for_short_snooze() {
    let r = Routine {
        snoozed_until: Some((now() + Duration::minutes(45)).timestamp() as u64),
        ..routine("a", "A", "claude", "0 * * * *", &["m"], &[], true)
    };
    assert_eq!(snooze_detail(&r, now()), "45m left");
}

#[test]
fn snooze_detail_shows_hours_left() {
    let r = Routine {
        snoozed_until: Some((now() + Duration::hours(3)).timestamp() as u64),
        ..routine("a", "A", "claude", "0 * * * *", &["m"], &[], true)
    };
    assert_eq!(snooze_detail(&r, now()), "3h left");
}

#[test]
fn snooze_detail_shows_days_left() {
    let r = Routine {
        snoozed_until: Some((now() + Duration::days(2)).timestamp() as u64),
        ..routine("a", "A", "claude", "0 * * * *", &["m"], &[], true)
    };
    assert_eq!(snooze_detail(&r, now()), "2d left");
}

#[test]
fn snooze_detail_shows_skip_runs() {
    let r = Routine {
        skip_runs: Some(5),
        ..routine("a", "A", "claude", "0 * * * *", &["m"], &[], true)
    };
    assert_eq!(snooze_detail(&r, now()), "5 runs skipped");
}

#[test]
fn snooze_detail_skip_runs_singular() {
    let r = Routine {
        skip_runs: Some(1),
        ..routine("a", "A", "claude", "0 * * * *", &["m"], &[], true)
    };
    assert_eq!(snooze_detail(&r, now()), "1 run skipped");
}

#[test]
fn snooze_detail_empty_when_deadline_past() {
    let r = Routine {
        snoozed_until: Some((now() - Duration::hours(1)).timestamp() as u64),
        ..routine("a", "A", "claude", "0 * * * *", &["m"], &[], true)
    };
    assert_eq!(snooze_detail(&r, now()), "");
}

#[test]
fn is_routine_snoozed_true_when_deadline_in_future() {
    let r = Routine {
        snoozed_until: Some((now() + Duration::hours(1)).timestamp() as u64),
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert!(is_routine_snoozed(&r, now()));
}

#[test]
fn is_routine_snoozed_false_when_deadline_past() {
    let r = Routine {
        snoozed_until: Some((now() - Duration::hours(1)).timestamp() as u64),
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert!(!is_routine_snoozed(&r, now()));
}

#[test]
fn is_routine_snoozed_true_when_skip_runs_nonzero() {
    let r = Routine {
        skip_runs: Some(3),
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert!(is_routine_snoozed(&r, now()));
}

#[test]
fn is_routine_snoozed_false_when_skip_runs_zero() {
    let r = Routine {
        skip_runs: Some(0),
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert!(!is_routine_snoozed(&r, now()));
}

#[test]
fn health_snoozed_until_future_is_snoozed() {
    let r = Routine {
        agent_registered: true,
        snoozed_until: Some((now() + Duration::hours(1)).timestamp() as u64),
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert_eq!(routine_health(&r, now()), RoutineHealth::Snoozed);
}

#[test]
fn health_snoozed_until_past_is_healthy() {
    let r = Routine {
        agent_registered: true,
        snoozed_until: Some((now() - Duration::hours(1)).timestamp() as u64),
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert_eq!(routine_health(&r, now()), RoutineHealth::Healthy);
}

#[test]
fn health_skip_runs_above_zero_is_snoozed() {
    let r = Routine {
        agent_registered: true,
        skip_runs: Some(2),
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert_eq!(routine_health(&r, now()), RoutineHealth::Snoozed);
}

#[test]
fn health_skip_runs_zero_is_healthy() {
    let r = Routine {
        agent_registered: true,
        skip_runs: Some(0),
        ..routine("a", "A", "claude", "0 * * * *", &["machine1"], &[], true)
    };
    assert_eq!(routine_health(&r, now()), RoutineHealth::Healthy);
}

#[test]
fn health_priority_order_dormant_most_urgent() {
    assert!(
        RoutineHealth::Dormant.priority() < RoutineHealth::DeadSchedule.priority(),
        "Dormant must outrank DeadSchedule"
    );
    assert!(RoutineHealth::DeadSchedule.priority() < RoutineHealth::AgentMissing.priority());
    assert!(RoutineHealth::AgentMissing.priority() < RoutineHealth::Disabled.priority());
    assert!(RoutineHealth::Disabled.priority() < RoutineHealth::Snoozed.priority());
    assert!(RoutineHealth::Snoozed.priority() < RoutineHealth::Healthy.priority());
}

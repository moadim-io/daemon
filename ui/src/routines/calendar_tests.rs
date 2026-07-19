use super::*;
use chrono::{NaiveDate, TimeZone};

/// Build a routine with only the fields `day_fire_rows` reads; the rest are inert.
fn routine(id: &str, title: &str, schedule: &str, enabled: bool) -> Routine {
    Routine {
        id: id.into(),
        title: title.into(),
        agent: "claude".into(),
        model: None,
        schedule: schedule.into(),
        prompt: String::new(),
        repositories: vec![],
        machines: vec![],
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

/// A fixed reference instant used wherever `day_fire_rows` needs a `now` to derive
/// snoozed status from.
fn fixed_now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 6, 21, 8, 0, 0).unwrap()
}

// ── ics_feed_url ────────────────────────────────────────────────────────────────

#[test]
fn ics_feed_url_joins_origin_and_path() {
    assert_eq!(
        ics_feed_url("https://moadim.example.com"),
        "https://moadim.example.com/api/v1/routines.ics"
    );
}

#[test]
fn ics_feed_url_preserves_port() {
    assert_eq!(
        ics_feed_url("http://localhost:8787"),
        "http://localhost:8787/api/v1/routines.ics"
    );
}

// ── day_fire_rows / day_title ─────────────────────────────────────────────────

#[test]
fn day_fire_rows_sorted_chronologically_across_routines() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    let routines = vec![
        routine("a", "Afternoon Routine", "0 14 * * *", true),
        routine("b", "Morning Routine", "0 9 * * *", true),
    ];
    let rows = day_fire_rows(&routines, day, fixed_now());
    assert_eq!(
        rows,
        vec![
            (
                "b".to_string(),
                "Morning Routine".to_string(),
                "09:00".to_string(),
                false
            ),
            (
                "a".to_string(),
                "Afternoon Routine".to_string(),
                "14:00".to_string(),
                false
            ),
        ]
    );
}

#[test]
fn day_fire_rows_skips_disabled_routines() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    let routines = vec![routine("a", "Disabled Routine", "0 9 * * *", false)];
    assert!(day_fire_rows(&routines, day, fixed_now()).is_empty());
}

#[test]
fn day_fire_rows_empty_when_nothing_fires() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    let routines = vec![routine("a", "Invalid", "not a cron", true)];
    assert!(day_fire_rows(&routines, day, fixed_now()).is_empty());
}

#[test]
fn day_fire_rows_flags_a_routine_snoozed_by_deadline() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    let now = fixed_now();
    let mut r = routine("a", "Snoozed Routine", "0 9 * * *", true);
    r.snoozed_until = Some((now.timestamp() + 3_600) as u64);
    let rows = day_fire_rows(&[r], day, now);
    assert_eq!(
        rows,
        vec![(
            "a".to_string(),
            "Snoozed Routine".to_string(),
            "09:00".to_string(),
            true
        )]
    );
}

#[test]
fn day_fire_rows_flags_a_routine_snoozed_by_skip_runs() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    let now = fixed_now();
    let mut r = routine("a", "Skip Routine", "0 9 * * *", true);
    r.skip_runs = Some(2);
    let rows = day_fire_rows(&[r], day, now);
    assert!(rows[0].3);
}

#[test]
fn day_fire_rows_not_flagged_once_snooze_deadline_has_passed() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    let now = fixed_now();
    let mut r = routine("a", "Past Snooze", "0 9 * * *", true);
    r.snoozed_until = Some((now.timestamp() - 3_600) as u64);
    let rows = day_fire_rows(&[r], day, now);
    assert!(!rows[0].3);
}

#[test]
fn day_title_formats_month_day_year() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    assert_eq!(day_title(day), "JUNE 21, 2026");
}

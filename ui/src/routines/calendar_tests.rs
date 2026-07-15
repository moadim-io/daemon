use super::*;
use chrono::NaiveDate;

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
    let rows = day_fire_rows(&routines, day);
    assert_eq!(
        rows,
        vec![
            (
                "b".to_string(),
                "Morning Routine".to_string(),
                "09:00".to_string()
            ),
            (
                "a".to_string(),
                "Afternoon Routine".to_string(),
                "14:00".to_string()
            ),
        ]
    );
}

#[test]
fn day_fire_rows_skips_disabled_routines() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    let routines = vec![routine("a", "Disabled Routine", "0 9 * * *", false)];
    assert!(day_fire_rows(&routines, day).is_empty());
}

#[test]
fn day_fire_rows_empty_when_nothing_fires() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    let routines = vec![routine("a", "Invalid", "not a cron", true)];
    assert!(day_fire_rows(&routines, day).is_empty());
}

#[test]
fn day_title_formats_month_day_year() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
    assert_eq!(day_title(day), "JUNE 21, 2026");
}

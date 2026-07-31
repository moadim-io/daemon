#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::model::{new_store, Routine};
use chrono::{Local, TimeZone};

fn routine_with(id: &str, schedule: &str, enabled: bool) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: schedule.to_string(),
        schedules: vec![],
        title: "My Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![],
        enabled,
        source: "managed".to_string(),
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

fn fixed_now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// A unique, freshly-created scratch directory under the system temp dir. `svc_ical`/
/// `svc_ical_routine` reload the store from this dir before rendering, so tests persist their
/// routines here to exercise the real reload in isolation.
fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-ical-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write `routine` to `{base}/{routine.id}/routine.toml` plus schedule.cron so the
/// directory-aware reload loads it back keyed by the `id` inside the file.
fn write_routine_to(base: &std::path::Path, routine: &Routine) {
    let dir = base.join(&routine.id);
    std::fs::create_dir_all(&dir).unwrap();
    let toml = format!(
        "id = \"{}\"\ntitle = \"{}\"\nagent = \"{}\"\nprompt = \"{}\"\nenabled = {}\ncreated_at = 0\nupdated_at = 0\nmachines = []\ntags = []\n",
        routine.id, routine.title, routine.agent, routine.prompt, routine.enabled,
    );
    std::fs::write(dir.join("routine.toml"), toml).unwrap();
    std::fs::write(dir.join("schedule.cron"), format!("{}\n", routine.schedule)).unwrap();
}

#[test]
fn empty_feed_has_only_calendar_wrapper() {
    let ics = build_ical(&[], fixed_now());
    assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
    assert!(ics.contains("VERSION:2.0\r\n"));
    assert!(ics.contains("PRODID:-//moadim//routines//EN\r\n"));
    assert!(ics.contains("X-WR-CALNAME:Moadim Routines\r\n"));
    assert!(ics.contains("REFRESH-INTERVAL;VALUE=DURATION:PT1H\r\n"));
    assert!(ics.contains("X-PUBLISHED-TTL:PT1H\r\n"));
    assert!(ics.ends_with("END:VCALENDAR\r\n"));
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}

#[test]
fn enabled_daily_routine_yields_events_within_horizon() {
    let ics = build_ical(&[routine_with("r1", "@daily", true)], fixed_now());
    let events = count(&ics, "BEGIN:VEVENT");
    // ~30 daily fires fall inside the 30-day horizon; allow slack for DST edges.
    assert!(events >= 28, "expected ~30 daily events, got {events}");
    assert!(ics.contains("SUMMARY:My Routine\r\n"));
    assert!(ics.contains("DESCRIPTION:do the thing (agent: claude)\r\n"));
    assert!(ics.contains("UID:r1-"));
    assert!(ics.contains("@moadim\r\n"));
    assert!(ics.contains("DTSTART"));
    assert!(ics.contains("DTSTAMP:"));
    // Fire times are momentary triggers, not busy blocks: every event is
    // TRANSPARENT so subscribers aren't marked BUSY (one per VEVENT).
    assert!(ics.contains("TRANSP:TRANSPARENT\r\n"));
    assert_eq!(count(&ics, "TRANSP:TRANSPARENT"), events);
}

#[test]
fn every_event_carries_a_duration() {
    // RFC 5545 requires each VEVENT to specify either DTEND or DURATION, otherwise
    // calendar clients render it as a zero-length instant. Every fire must emit one —
    // including the trailing truncation-marker VEVENT, so use a capped ("* * * * *")
    // schedule that emits both.
    let ics = build_ical(&[routine_with("r1", "* * * * *", true)], fixed_now());
    assert_eq!(
        count(&ics, "BEGIN:VEVENT"),
        count(&ics, "DURATION:PT15M"),
        "each VEVENT, including the truncation marker, should carry exactly one DURATION line"
    );
}

#[test]
fn events_are_transparent_to_free_busy() {
    // The feed is informational: a fire must not consume the subscriber's
    // free/busy time (RFC 5545 §3.8.2.7 defaults TRANSP to OPAQUE = busy). Use a
    // capped schedule so the truncation-marker VEVENT is covered too.
    let ics = build_ical(&[routine_with("r1", "* * * * *", true)], fixed_now());
    let events = count(&ics, "BEGIN:VEVENT");
    assert!(events > 0, "expected at least one event");
    // Exactly one TRANSP:TRANSPARENT (and Outlook free-busy hint) per VEVENT.
    assert_eq!(count(&ics, "TRANSP:TRANSPARENT\r\n"), events);
    assert_eq!(count(&ics, "X-MICROSOFT-CDO-BUSYSTATUS:FREE\r\n"), events);
}

#[test]
fn disabled_routine_contributes_nothing() {
    let ics = build_ical(&[routine_with("r1", "@daily", false)], fixed_now());
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}

#[test]
fn unparseable_schedule_is_skipped() {
    let ics = build_ical(&[routine_with("r1", "@reboot", true)], fixed_now());
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}

#[test]
fn power_saving_routine_contributes_nothing() {
    // power_saving is an independent signal from enabled (see svc_trigger_scheduled),
    // and blocks a scheduled fire the same way; the feed must honor it too.
    let mut routine = routine_with("r1", "@daily", true);
    routine.power_saving = true;
    let ics = build_ical(&[routine], fixed_now());
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}
include!("snoozed_routine_skips_fires_before_the_deadline_tests.rs");

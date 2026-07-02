//! Host-side unit tests for the schedule heatmap's pure aggregation logic: the
//! 7×24 fire-density grid, the color-ramp bucketing, the axis totals, and the
//! derived "busiest window" / day labels. All deterministic given a fixed `now`.

use super::*;
use chrono::{Local, TimeZone};

/// A fixed reference instant: Mon 2026-06-22 10:00:00 local. Off midnight and
/// noon so per-day schedules land unambiguously inside the window.
fn now() -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 6, 22, 10, 0, 0)
        .single()
        .expect("valid local time")
}

/// `now`'s calendar day, the grid's row-0 anchor.
fn today() -> NaiveDate {
    now().date_naive()
}

fn source(kind: Kind, schedule: &str, enabled: bool) -> HeatSource {
    HeatSource {
        kind,
        schedule: schedule.into(),
        enabled,
    }
}

// ─── HeatFilter ────────────────────────────────────────────────────────────

#[test]
fn filter_accepts_by_kind() {
    assert!(HeatFilter::All.accepts(Kind::Routine));
    assert!(HeatFilter::Routine.accepts(Kind::Routine));
}

#[test]
fn filter_labels() {
    assert_eq!(HeatFilter::All.label(), "ALL");
    assert_eq!(HeatFilter::Routine.label(), "ROUTINES");
}

// ─── compute_heatmap ───────────────────────────────────────────────────────

#[test]
fn daily_noon_schedule_fills_one_cell_per_day() {
    // From 10:00 today, "every day at 12:00" fires once on each of the 7 days
    // in the window (today's noon is still ahead), all in the hour-12 column.
    let sources = vec![source(Kind::Routine, "0 12 * * *", true)];
    let map = compute_heatmap(&sources, now(), HeatFilter::All);

    assert_eq!(map.total, 7);
    assert_eq!(map.max_cell, 1);
    assert_eq!(map.peak, Some((0, 12)));
    for day in 0..HEAT_DAYS {
        assert_eq!(map.grid[day][12], 1, "day {day} noon");
        assert_eq!(map.grid[day][0], 0, "day {day} midnight empty");
    }
}

#[test]
fn elapsed_hours_today_stay_empty() {
    // "Every day at 08:00" — 08:00 today already passed at 10:00, so today's
    // row is empty while the other six days each get one fire.
    let sources = vec![source(Kind::Routine, "0 8 * * *", true)];
    let map = compute_heatmap(&sources, now(), HeatFilter::All);

    assert_eq!(map.grid[0][8], 0, "today's 08:00 already elapsed");
    assert_eq!(map.total, 6);
    assert_eq!(map.peak, Some((1, 8)));
}

#[test]
fn disabled_sources_are_ignored() {
    let sources = vec![source(Kind::Routine, "0 12 * * *", false)];
    let map = compute_heatmap(&sources, now(), HeatFilter::All);
    assert_eq!(map.total, 0);
    assert!(map.peak.is_none());
}

#[test]
fn far_future_schedule_outside_window_counts_zero() {
    // 1 January fires well beyond the 7-day window from late June.
    let sources = vec![source(Kind::Routine, "0 0 1 1 *", true)];
    let map = compute_heatmap(&sources, now(), HeatFilter::All);
    assert_eq!(map.total, 0);
}

#[test]
fn invalid_schedule_is_skipped() {
    let sources = vec![
        source(Kind::Routine, "not a cron", true),
        source(Kind::Routine, "0 12 * * *", true),
    ];
    let map = compute_heatmap(&sources, now(), HeatFilter::All);
    assert_eq!(map.total, 7);
}

#[test]
fn filter_restricts_counted_sources() {
    let sources = vec![
        source(Kind::Routine, "0 12 * * *", true),
        source(Kind::Routine, "0 12 * * *", true),
    ];
    assert_eq!(compute_heatmap(&sources, now(), HeatFilter::All).total, 14);
    assert_eq!(
        compute_heatmap(&sources, now(), HeatFilter::Routine).total,
        14
    );
}

#[test]
fn collisions_stack_in_one_cell_and_set_the_peak() {
    // Two daily-noon schedules pile two fires into each noon cell.
    let sources = vec![
        source(Kind::Routine, "0 12 * * *", true),
        source(Kind::Routine, "0 12 * * *", true),
    ];
    let map = compute_heatmap(&sources, now(), HeatFilter::All);
    assert_eq!(map.max_cell, 2);
    assert_eq!(map.grid[0][12], 2);
    assert_eq!(map.peak, Some((0, 12)));
}

#[test]
fn empty_sources_produce_a_zeroed_grid() {
    let map = compute_heatmap(&[], now(), HeatFilter::All);
    assert_eq!(map.grid.len(), HEAT_DAYS);
    assert_eq!(map.grid[0].len(), HEAT_HOURS);
    assert_eq!(map.total, 0);
    assert_eq!(map.max_cell, 0);
    assert!(map.peak.is_none());
}

// ─── intensity_level ───────────────────────────────────────────────────────

#[test]
fn intensity_level_buckets_into_five_steps() {
    assert_eq!(intensity_level(0, 4), 0);
    assert_eq!(intensity_level(5, 0), 0); // guard: zero max never divides
    assert_eq!(intensity_level(1, 4), 1);
    assert_eq!(intensity_level(2, 4), 2);
    assert_eq!(intensity_level(3, 4), 3);
    assert_eq!(intensity_level(4, 4), 4);
    assert_eq!(intensity_level(1, 100), 1); // tiny ratio still reaches step 1
    assert_eq!(intensity_level(100, 100), 4);
}

// ─── axis totals ───────────────────────────────────────────────────────────

#[test]
fn day_and_hour_totals_sum_the_grid() {
    let sources = vec![
        source(Kind::Routine, "0 12 * * *", true),
        source(Kind::Routine, "0 12 * * *", true),
    ];
    let map = compute_heatmap(&sources, now(), HeatFilter::All);

    let days = day_totals(&map);
    assert_eq!(days.len(), HEAT_DAYS);
    assert!(days.iter().all(|&d| d == 2)); // two noon fires each day

    let hours = hour_totals(&map);
    assert_eq!(hours.len(), HEAT_HOURS);
    assert_eq!(hours[12], 14); // every day's two noon fires land in hour 12
    assert_eq!(hours[0], 0);
}

// ─── peak_label / day_label ──────────────────────────────────────────────────

#[test]
fn peak_label_reads_weekday_hour_and_count() {
    let single = compute_heatmap(
        &[source(Kind::Routine, "0 12 * * *", true)],
        now(),
        HeatFilter::All,
    );
    assert_eq!(
        peak_label(&single, today()).as_deref(),
        Some("MON 12:00 · 1 run")
    );

    let double = compute_heatmap(
        &[
            source(Kind::Routine, "0 12 * * *", true),
            source(Kind::Routine, "0 12 * * *", true),
        ],
        now(),
        HeatFilter::All,
    );
    assert_eq!(
        peak_label(&double, today()).as_deref(),
        Some("MON 12:00 · 2 runs")
    );
}

#[test]
fn peak_label_is_none_for_empty_grid() {
    let map = compute_heatmap(&[], now(), HeatFilter::All);
    assert!(peak_label(&map, today()).is_none());
}

#[test]
fn day_label_counts_weekdays_forward_from_today() {
    assert_eq!(day_label(today(), 0), "MON 22");
    assert_eq!(day_label(today(), 1), "TUE 23");
    assert_eq!(day_label(today(), 6), "SUN 28");
}

// ─── record → source mappers ─────────────────────────────────────────────────

fn routine(schedule: &str, enabled: bool) -> Routine {
    Routine {
        id: "rid".into(),
        schedule: schedule.into(),
        title: "t".into(),
        agent: "a".into(),
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

#[test]
fn sources_of_maps_records_preserving_kind_and_enabled() {
    let routines = vec![routine("0 0 * * *", false)];
    let sources = sources_of(&routines);

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].kind, Kind::Routine);
    assert_eq!(sources[0].schedule, "0 0 * * *");
    assert!(!sources[0].enabled);
}

#[test]
fn filled_cells_counts_nonempty_cells_only() {
    let map = compute_heatmap(
        &[source(Kind::Routine, "0 12 * * *", true)],
        now(),
        HeatFilter::All,
    );
    // One non-empty cell (hour 12) on each of the 7 days.
    assert_eq!(filled_cells(&map), HEAT_DAYS as u32);
}

//! Pure aggregation logic behind the schedule heatmap: the 7×24 fire-density
//! grid, the color-ramp bucketing, the axis totals, and the derived
//! "busiest window" / day labels. Split out of `schedule_heatmap.rs` to keep
//! that file under the line-count gate; the component in that file is a thin
//! shell over these host-testable functions (see `schedule_heatmap_tests.rs`).

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Timelike};

use crate::overview::Kind;
use crate::parse_cron;
use crate::routines::Routine;

/// Rows in the grid: the next 7 calendar days, row 0 = today.
pub(crate) const HEAT_DAYS: usize = 7;
/// Columns in the grid: the 24 hours of the day.
pub(crate) const HEAT_HOURS: usize = 24;
/// Upper bound on fire-time iterations per source over the window. An
/// every-minute schedule fires 7×1440 = 10 080 times/week; this leaves headroom
/// while bounding cost on pathological (e.g. per-second) inputs.
const MAX_FIRES_PER_SOURCE: usize = 20_000;

/// Weekday abbreviations indexed by [`chrono::Weekday::num_days_from_sunday`].
const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

/// Source-kind filter for the grid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HeatFilter {
    /// Count all sources (currently just routines).
    All,
    /// Count routines only.
    Routine,
}

impl HeatFilter {
    /// Whether a source of `kind` is counted under this filter.
    pub(crate) fn accepts(self, kind: Kind) -> bool {
        match self {
            Self::All => true,
            Self::Routine => kind == Kind::Routine,
        }
    }

    /// Short uppercase label for the toggle button.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Routine => "ROUTINES",
        }
    }
}

/// A schedule-bearing entity reduced to just what the heatmap needs. `Routine`
/// maps onto this so the aggregation stays agnostic of its full shape (and
/// host-testable without wasm types).
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct HeatSource {
    /// Always `Kind::Routine` for now.
    pub kind: Kind,
    /// Raw cron expression used to compute fire times.
    pub schedule: String,
    /// Whether the entity is currently enabled (disabled ones never fire).
    pub enabled: bool,
}

/// The computed 7×24 density grid plus derived stats.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Heatmap {
    /// `grid[day][hour]` = number of fires; day 0 = today.
    pub grid: Vec<Vec<u32>>,
    /// Total fires across the whole window.
    pub total: u32,
    /// Largest single-cell count — the color-ramp denominator.
    pub max_cell: u32,
    /// `(day, hour)` of the busiest cell, or `None` when nothing fires.
    pub peak: Option<(usize, usize)>,
    /// Number of enabled, filter-matching sources that contributed at least one fire.
    pub sources: u32,
}

/// Aggregate the next-7-day fire density of every enabled source matching
/// `filter`, bucketed by `(day, hour)` with day 0 = `now`'s calendar day. Fires
/// are counted strictly after `now`, so hours already elapsed today read empty.
pub(crate) fn compute_heatmap(
    sources: &[HeatSource],
    now: DateTime<Local>,
    filter: HeatFilter,
) -> Heatmap {
    let today = now.date_naive();
    let end_date = today + Duration::days(HEAT_DAYS as i64);
    let mut grid = vec![vec![0_u32; HEAT_HOURS]; HEAT_DAYS];
    let mut sources_counted = 0_u32;

    for source in sources
        .iter()
        .filter(|s| s.enabled && filter.accepts(s.kind))
    {
        let Some(cron) = parse_cron(&source.schedule) else {
            continue;
        };
        let mut contributed = false;
        // `iter_after(now)` yields fires strictly after `now` in chronological
        // order, so each `date` is on or after `today`; stop at the first fire
        // that lands on or past the window's end. The take() caps cost on
        // pathological (sub-minute) schedules.
        for dt in cron.iter_after(now).take(MAX_FIRES_PER_SOURCE) {
            let date = dt.date_naive();
            if date >= end_date {
                break;
            }
            let day = (date - today).num_days() as usize;
            grid[day][dt.hour() as usize] += 1;
            contributed = true;
        }
        if contributed {
            sources_counted += 1;
        }
    }

    let mut total = 0_u32;
    let mut max_cell = 0_u32;
    let mut peak = None;
    for (day, hours) in grid.iter().enumerate() {
        for (hour, &count) in hours.iter().enumerate() {
            total += count;
            if count > max_cell {
                max_cell = count;
                peak = Some((day, hour));
            }
        }
    }

    Heatmap {
        grid,
        total,
        max_cell,
        peak,
        sources: sources_counted,
    }
}

/// The 0–4 color-ramp bucket for `count` relative to the grid's `max` cell.
/// 0 = empty; 1–4 split the non-empty range into quartiles so the busiest cells
/// reach the top of the ramp.
pub(crate) fn intensity_level(count: u32, max: u32) -> u8 {
    if count == 0 || max == 0 {
        return 0;
    }
    let ratio = f64::from(count) / f64::from(max);
    ((ratio * 4.0).ceil() as u8).clamp(1, 4)
}

/// Per-day fire totals (length [`HEAT_DAYS`]).
pub(crate) fn day_totals(map: &Heatmap) -> Vec<u32> {
    map.grid.iter().map(|hours| hours.iter().sum()).collect()
}

/// Per-hour fire totals across all days (length [`HEAT_HOURS`]).
pub(crate) fn hour_totals(map: &Heatmap) -> Vec<u32> {
    (0..HEAT_HOURS)
        .map(|hour| map.grid.iter().map(|day| day[hour]).sum())
        .collect()
}

/// Human label for the busiest window, e.g. "THU 14:00 · 3 runs", or `None`
/// when the grid is empty.
pub(crate) fn peak_label(map: &Heatmap, today: NaiveDate) -> Option<String> {
    map.peak.map(|(day, hour)| {
        let count = map.grid[day][hour];
        let plural = if count == 1 { "run" } else { "runs" };
        format!("{} {hour:02}:00 · {count} {plural}", weekday_of(today, day))
    })
}

/// `"MON 23"`-style label for grid row `day`, counting forward from `today`.
pub(crate) fn day_label(today: NaiveDate, day: usize) -> String {
    let date = today + Duration::days(day as i64);
    format!("{} {}", WEEKDAYS[weekday_index(date)], date.day())
}

/// Weekday abbreviation for the row `day` days after `today`.
fn weekday_of(today: NaiveDate, day: usize) -> &'static str {
    WEEKDAYS[weekday_index(today + Duration::days(day as i64))]
}

/// Index into [`WEEKDAYS`] for `date`.
fn weekday_index(date: NaiveDate) -> usize {
    date.weekday().num_days_from_sunday() as usize
}

/// Map a routine onto the shared heatmap source.
fn from_routine(routine: &Routine) -> HeatSource {
    HeatSource {
        kind: Kind::Routine,
        schedule: routine.schedule.clone(),
        enabled: routine.enabled,
    }
}

/// Map the routine record list into one `HeatSource` vector.
pub(crate) fn sources_of(routines: &[Routine]) -> Vec<HeatSource> {
    routines.iter().map(from_routine).collect()
}

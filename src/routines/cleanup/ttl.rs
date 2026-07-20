//! Retention (TTL) config for finished-run workbenches.
//!
//! A finished workbench is reaped once it is older than its routine's TTL (see the cleanup module).
//! Retention is kept short: a finished run is worth keeping only until the next run is due, and
//! never longer than [`MAX_TTL_SECS`]. So the effective TTL is `min(MAX_TTL_SECS, cron interval)`,
//! optionally lowered further by an explicit `ttl_secs`.

use chrono::Local;

use super::super::model::Routine;

/// Upper bound on how long a finished run's workbench is retained: one hour.
///
/// Also the fallback when a routine's schedule can't be parsed (e.g. `@reboot`) or its interval
/// can't be computed, and the retention for orphaned workbenches whose routine was since deleted.
pub const MAX_TTL_SECS: u64 = 60 * 60;

/// Cron-derived retention ceiling for a routine running on `schedule`:
/// `min(MAX_TTL_SECS, cron interval)`.
///
/// An explicit `ttl_secs` above this is silently clamped by [`Routine::effective_ttl_secs`], so
/// create/update validation rejects it instead (#468).
pub(crate) fn ttl_ceiling_secs(schedule: &str) -> u64 {
    MAX_TTL_SECS.min(cron_interval_secs(schedule).unwrap_or(MAX_TTL_SECS))
}

/// How many consecutive future fires to sample when hunting for `schedule`'s shortest gap.
/// Comfortably covers multi-fire-per-day schedules (the only ones where the gap can dip below
/// [`MAX_TTL_SECS`]) across a multi-day look-ahead window; iterating a cron schedule is cheap, so
/// there's no cost pressure to trim it further.
const GAP_SAMPLE_FIRES: usize = 48;

/// The minimum gap (seconds) between consecutive entries in `fires`, sampling up to
/// [`GAP_SAMPLE_FIRES`] of them. `None` if `fires` yields fewer than two timestamps.
///
/// Shared by [`cron_interval_secs`] so the sampling stays stable for unevenly-spaced schedules.
fn min_gap_secs(mut fires: impl Iterator<Item = chrono::DateTime<Local>>) -> Option<u64> {
    let mut prev = fires.next()?;
    let mut min_gap: Option<i64> = None;
    for next in fires.take(GAP_SAMPLE_FIRES) {
        let gap = (next - prev).num_seconds();
        min_gap = Some(min_gap.map_or(gap, |current_min| current_min.min(gap)));
        prev = next;
    }
    u64::try_from(min_gap?).ok()
}

/// The shortest gap between consecutive scheduled runs of `schedule`, or `None` if it can't be
/// parsed or fewer than two future fire times exist.
///
/// Takes the *minimum* gap across up to [`GAP_SAMPLE_FIRES`] consecutive fires after `now`,
/// rather than just the next two — for an unevenly-spaced schedule, "the next two fires from now"
/// alone depends on where `now` falls. E.g. `"0,30 9 * * *"` (fires at 09:00 and 09:30 daily) has
/// a true 30-minute minimum gap, but sampling only the next two fires gives 30 minutes when `now`
/// is just before 09:00 and ~23.5 hours when `now` is just after 09:00. Sampling multiple fires
/// keeps the result stable regardless of `now`, so sub-hour schedules (the only ones this
/// affects, since the result is only ever used via `MAX_TTL_SECS.min(..)`) really do have a
/// constant interval as callers assume.
pub(super) fn cron_interval_secs(schedule: &str) -> Option<u64> {
    let union = crate::utils::cron::compiled_union(schedule)?;
    let cron = union.iter().next()?.schedule();
    min_gap_secs(cron.after(&Local::now()))
}

impl Routine {
    /// Retention for this routine's finished workbenches.
    ///
    /// `min(MAX_TTL_SECS, cron interval)`, then further lowered by an explicit `ttl_secs` if set.
    /// An explicit `ttl_secs` can only shorten retention, never raise it above the cron-derived cap.
    pub fn effective_ttl_secs(&self) -> u64 {
        let ceiling = ttl_ceiling_secs(&self.schedule);
        self.ttl_secs.map_or(ceiling, |secs| secs.min(ceiling))
    }
}

//! Retention (TTL) config for finished-run workbenches.
//!
//! A finished workbench is reaped once it is older than its routine's TTL (see the cleanup module).
//! Retention is kept short: a finished run is worth keeping only until the next run is due, and
//! never longer than [`MAX_TTL_SECS`]. So the effective TTL is `min(MAX_TTL_SECS, cron interval)`,
//! optionally lowered further by an explicit `ttl_secs`.

use chrono::Local;
use croner::Cron;

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

/// Seconds between the next two scheduled runs of `schedule`, or `None` if it can't be parsed or two
/// future fire times can't be computed. For irregular schedules this is the interval starting now;
/// since it only matters when below [`MAX_TTL_SECS`], sub-hour schedules (the only ones it changes)
/// have a constant interval regardless of `now`.
pub(super) fn cron_interval_secs(schedule: &str) -> Option<u64> {
    if let Some(union) = crate::utils::cron::compiled_union(schedule) {
        let cron = union.iter().next()?.schedule();
        let mut fires = cron.after(&Local::now());
        let first = fires.next()?;
        let second = fires.next()?;
        return u64::try_from((second - first).num_seconds()).ok();
    }
    let cron = schedule.parse::<Cron>().ok()?;
    let mut fires = cron.iter_after(Local::now());
    let first = fires.next()?;
    let second = fires.next()?;
    u64::try_from((second - first).num_seconds()).ok()
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

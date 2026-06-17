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

/// Default cap on a single run's wall-clock runtime when a routine sets no explicit
/// `max_runtime_secs`: six hours.
///
/// Generous enough not to interrupt a legitimately long agent run, while still bounding a hung
/// session (one that waits on stdin, loops forever, or blocks on a stuck network/git operation) so
/// it cannot accumulate one zombie per cron tick. The watchdog in the cleanup module kills any live
/// session whose run has exceeded its routine's [`Routine::effective_max_runtime_secs`]. It is also
/// the fallback for orphaned workbenches whose routine was since deleted.
pub const DEFAULT_MAX_RUNTIME_SECS: u64 = 6 * 60 * 60;

impl Routine {
    /// Maximum wall-clock seconds a single run of this routine may execute before the watchdog
    /// force-kills its session: the explicit `max_runtime_secs` if set, else
    /// [`DEFAULT_MAX_RUNTIME_SECS`].
    pub fn effective_max_runtime_secs(&self) -> u64 {
        self.max_runtime_secs.unwrap_or(DEFAULT_MAX_RUNTIME_SECS)
    }

    /// Retention for this routine's finished workbenches.
    ///
    /// `min(MAX_TTL_SECS, cron interval)`, then further lowered by an explicit `ttl_secs` if set.
    /// An explicit `ttl_secs` can only shorten retention, never raise it above the cron-derived cap.
    pub fn effective_ttl_secs(&self) -> u64 {
        let ceiling = MAX_TTL_SECS.min(self.cron_interval_secs().unwrap_or(MAX_TTL_SECS));
        self.ttl_secs.map_or(ceiling, |secs| secs.min(ceiling))
    }

    /// Seconds between the next two scheduled runs, or `None` if the schedule can't be parsed or two
    /// future fire times can't be computed. For irregular schedules this is the interval starting
    /// now; since it only matters when below [`MAX_TTL_SECS`], sub-hour schedules (the only ones it
    /// changes) have a constant interval regardless of `now`.
    fn cron_interval_secs(&self) -> Option<u64> {
        let cron = self.schedule.parse::<Cron>().ok()?;
        let mut fires = cron.iter_after(Local::now());
        let first = fires.next()?;
        let second = fires.next()?;
        u64::try_from((second - first).num_seconds()).ok()
    }
}

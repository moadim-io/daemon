//! Max-runtime (watchdog) config for in-flight routine runs.
//!
//! A run launches its agent in a detached tmux session that nothing otherwise bounds: a hung agent
//! (waiting on stdin, looping, blocked on a stuck network/git op) never exits, so the TTL reaper —
//! which only touches *finished* runs — can never reap it, and each cron tick stacks another zombie
//! session + workbench. The cleanup watchdog force-kills any session whose run has exceeded its
//! routine's max runtime, after which the workbench is reaped under the normal TTL rules.

use super::super::model::Routine;

/// Upper bound on a single run's wall-clock duration before the watchdog kills its session: 1h.
///
/// Mirrors the TTL cap ([`super::ttl::MAX_TTL_SECS`]): a run is only worth keeping alive until the
/// next run is due, and never longer than this. Also the fallback for orphaned workbenches whose
/// routine was since deleted.
pub const MAX_RUNTIME_SECS: u64 = 60 * 60;

/// Cron-derived watchdog ceiling for a routine running on `schedule`:
/// `min(MAX_RUNTIME_SECS, cron interval)`.
///
/// An explicit `max_runtime_secs` above this is silently clamped by
/// [`Routine::effective_max_runtime_secs`], so create/update validation rejects it instead (#468).
pub(crate) fn max_runtime_ceiling_secs(schedule: &str) -> u64 {
    MAX_RUNTIME_SECS.min(super::ttl::cron_interval_secs(schedule).unwrap_or(MAX_RUNTIME_SECS))
}

impl Routine {
    /// Effective max runtime for a single run of this routine.
    ///
    /// Mirrors [`Routine::effective_ttl_secs`]: `min(MAX_RUNTIME_SECS, cron interval)`, then further
    /// lowered by an explicit `max_runtime_secs` if set. An explicit value can only shorten the
    /// bound, never raise it above the cron-derived cap.
    pub fn effective_max_runtime_secs(&self) -> u64 {
        let ceiling = max_runtime_ceiling_secs(&self.schedule);
        self.max_runtime_secs
            .map_or(ceiling, |secs| secs.min(ceiling))
    }
}

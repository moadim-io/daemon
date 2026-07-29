//! The failure circuit-breaker (issue #521): a routine that fails every scheduled fire otherwise
//! keeps spawning a fresh agent session forever — on a `*/5` cron that is ~288 doomed sessions/day,
//! burning CPU, disk, and API spend with no automatic backstop. This module tracks each routine's
//! [`Routine::consecutive_failures`](crate::routines::model::Routine::consecutive_failures) and,
//! once it reaches the routine's opt-in
//! [`Routine::failure_threshold`](crate::routines::model::Routine::failure_threshold),
//! auto-disables it through the same `enabled = false` path a user
//! flipping the toggle off would use — so the very next crontab sync removes it, and no further
//! sessions spawn.
//!
//! # Hook point: the TTL-reap `persist` closure, not `svc_list_runs`
//!
//! A run's outcome becomes knowable in two places: the on-demand `service_runs`
//! listing (`svc_list_runs`/`run_summary`), computed fresh on every `GET .../runs` call straight
//! from the workbench's `exit_code` file and tmux liveness; and the periodic TTL-reap sweep
//! ([`super::cleanup_expired_workbenches`]'s `persist` closure), which already durably records the
//! same outcome into `runs.log` right before the workbench is removed (see
//! [`super::super::run_history`]).
//!
//! `svc_list_runs` looks tempting for responsiveness — it observes a finished run immediately, not
//! after a TTL — but it is *pull-based*: it only ever runs when an API client happens to call it, so
//! a routine nobody is watching would never trip the breaker at all. The reap sweep, by contrast, is
//! a background task the daemon already drives on its own (every [`super::CLEANUP_INTERVAL`], 5
//! minutes) independent of anyone polling the API, which is what "auto-disables itself" requires.
//! It also comes with [`super::super::run_history::has_persisted_run`] already guarding against
//! recording (and so double-counting) the same finished run twice — reusing it here needs no new
//! dedup bookkeeping.
//!
//! The tradeoff is that this hook only fires once a workbench's *retention* has elapsed, not the
//! instant its session exits. In the worst case that sounds like it could be a long delay, but
//! retention is `min(MAX_TTL_SECS, cron interval)` ([`super::ttl`]) — capped at the routine's own
//! firing interval — so for exactly the high-frequency routines this feature is meant to protect
//! (e.g. the `*/5` example in #521) the delay before a finished run is counted is bounded by
//! [`super::CLEANUP_INTERVAL`] (a handful of minutes), not by the retention window itself. That is
//! responsive enough to stop the bleed within a few extra fires — a world away from "forever" — at
//! the cost of no new per-workbench marker files or a second background sweep.

use crate::routine_storage::write_routine;
use crate::routines::model::{RoutineStore, RunStatus};
use crate::utils::lock::LockRecover;

/// Update routine `id`'s consecutive-failure streak for a run that just durably finished with
/// `status`, auto-disabling it once/if [`Routine::failure_threshold`](
/// crate::routines::model::Routine::failure_threshold) is reached.
///
/// `status` is never [`RunStatus::Running`] here (the caller only invokes this for a workbench
/// already confirmed finished — see this module's doc comment). [`RunStatus::Success`] resets the
/// streak to `0`; anything else (`Failed` or `Unknown`) increments it — `Unknown` counts too because
/// it covers a session that vanished with no exit code, including one the max-runtime watchdog just
/// force-killed for hanging, which is exactly the kind of never-succeeds loop this breaker exists to
/// stop. A missing `id` (routine deleted between the workbench's creation and this sweep) is a no-op.
///
/// Persists the routine unconditionally (mirroring how [`super::super::run_history`] persists every
/// outcome), but only re-syncs the crontab when this call is the one that actually flips `enabled`
/// to `false` — every other call (the common case: a success, or a failure short of the threshold)
/// changes nothing the crontab cares about, so it skips that round trip.
pub(super) fn record_run_outcome(store: &RoutineStore, id: &str, status: RunStatus) {
    let mut just_disabled = false;
    let routine = {
        let mut lock = store.lock_recover();
        let Some(routine) = lock.get_mut(id) else {
            return;
        };
        if status == RunStatus::Success {
            routine.consecutive_failures = 0;
        } else {
            routine.consecutive_failures = routine.consecutive_failures.saturating_add(1);
            // `None`/`Some(0)` opts out, preserving today's retry-forever behavior (#521).
            if let Some(threshold) = routine.failure_threshold.filter(|&threshold| threshold > 0) {
                if routine.enabled && routine.consecutive_failures >= threshold {
                    routine.enabled = false;
                    routine.auto_disabled_reason = Some(format!(
                        "auto-disabled after {threshold} consecutive failed run(s); \
                         re-enable to reset and try again"
                    ));
                    just_disabled = true;
                }
            }
        }
        routine.clone()
    };
    if let Err(err) = write_routine(&routine) {
        log::warn!("circuit breaker: failed to persist routine {id:?}: {err}");
    }
    if just_disabled {
        log::warn!(
            "circuit breaker: auto-disabled routine {id:?} after {} consecutive failed run(s)",
            routine.consecutive_failures
        );
        if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
            log::warn!("circuit breaker: crontab sync after auto-disable failed: {err}");
        }
    }
}

#[cfg(test)]
#[path = "circuit_breaker_tests.rs"]
mod circuit_breaker_tests;

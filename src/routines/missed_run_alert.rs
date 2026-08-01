#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;
use chrono::TimeZone;

/// Small grace window so a routine scheduled for the current minute does not flash as "missed"
/// before cron has had a chance to call `/scheduled-trigger` and append `scheduled.log`.
const MISSED_RUN_GRACE_SECS: u64 = 60;

/// Latest scheduled fire that elapsed without a matching scheduled trigger record.
///
/// This is an alert-only signal: it never spawns a catch-up run. The baseline is the last scheduled
/// trigger when present, otherwise the routine creation timestamp, so existing never-run routines
/// can still surface missed windows without inventing a run record.
pub(crate) fn missed_scheduled_run_at(routine: &Routine, schedules: &[String]) -> Option<u64> {
    missed_scheduled_run_at_now(
        routine,
        schedules,
        Local::now().timestamp().try_into().ok()?,
    )
}

pub(crate) fn missed_scheduled_run_at_now(
    routine: &Routine,
    schedules: &[String],
    now_secs: u64,
) -> Option<u64> {
    if !eligible_for_missed_run_alert(routine, now_secs) {
        return None;
    }
    let latest_allowed = now_secs.checked_sub(MISSED_RUN_GRACE_SECS)?;
    let baseline = routine
        .last_scheduled_trigger_at
        .unwrap_or(routine.created_at);
    if baseline >= latest_allowed {
        return None;
    }
    let baseline_i64 = i64::try_from(baseline).ok()?;
    let latest_i64 = i64::try_from(latest_allowed).ok()?.checked_add(1)?;
    let baseline_dt = Local.timestamp_opt(baseline_i64, 0).single()?;
    let latest_dt = Local.timestamp_opt(latest_i64, 0).single()?;
    schedules
        .iter()
        .filter_map(|schedule| {
            crate::utils::cron::normalize_schedule(schedule)
                .parse::<Cron>()
                .ok()
        })
        .filter_map(|cron| cron.iter_before(latest_dt).next())
        .filter(|fire| *fire > baseline_dt)
        .filter_map(|fire| u64::try_from(fire.timestamp()).ok())
        .max()
}

fn eligible_for_missed_run_alert(routine: &Routine, now_secs: u64) -> bool {
    if !routine.enabled || crate::global_lock::is_globally_locked() {
        return false;
    }
    if !crate::machine::targets(&routine.machines, &crate::machine::current_machine()) {
        return false;
    }
    if routine.snoozed_until.is_some_and(|until| now_secs < until) {
        return false;
    }
    !routine.skip_runs.is_some_and(|runs| runs > 0)
}

#[cfg(test)]
#[path = "missed_run_alert_tests.rs"]
mod missed_run_alert_tests;

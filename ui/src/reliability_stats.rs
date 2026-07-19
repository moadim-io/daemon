//! Pure reliability-metrics computation for the RELIABILITY page: per-routine success rate,
//! active pass/fail streak, and flakiness (status-flip rate) over each routine's most recent
//! finished runs. Split from `reliability.rs` (the page shell) so the math stays host-testable
//! without a wasm/DOM dependency, mirroring `overview.rs`'s split from `overview_tests.rs`.
//!
//! Best practice (CI/CD reliability dashboards — GitHub Actions Insights, `CircleCI` Insights,
//! Datadog Test Visibility): rank jobs by recent success rate and flag flaky ones (alternating
//! pass/fail) separately from steadily-failing ones, since the two call for different responses
//! — a flaky routine needs investigation, a steadily-failing one needs immediate attention.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::routines::{FleetRunSummary, RunStatus};

/// Most recent finished runs per routine considered for reliability metrics. Caps memory/render
/// cost per routine the same way `sparkline.rs`'s `SPARKLINE_LEN` caps its tick strip, just with
/// a longer window since flakiness needs more samples than an at-a-glance sparkline does.
pub(crate) const SAMPLE_LEN: usize = 20;

/// Minimum finished-run sample before flakiness is judged — a 2-run sample that flips once is
/// noise, not a signal.
const FLAKY_MIN_SAMPLE: usize = 5;

/// A routine whose adjacent-run status flips make up at least this fraction of its sample's
/// adjacent pairs is flagged flaky.
const FLAKY_FLIP_RATIO: f64 = 0.4;

/// A routine's most-recent-run streak: consecutive finished runs, newest first, sharing the same
/// outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Streak {
    /// `n` consecutive successes (n >= 1), most recent run first.
    Success(usize),
    /// `n` consecutive failures (n >= 1), most recent run first.
    Failure(usize),
    /// No finished run in the sample.
    None,
}

/// Reliability metrics for one routine, derived from its most recent finished
/// (`Success`/`Failed`) runs — `Running`/`Unknown` runs are excluded since they say nothing
/// about reliability.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RoutineReliability {
    pub routine_id: String,
    pub routine_title: String,
    /// Finished runs considered, out of up to `SAMPLE_LEN`.
    pub sample_size: usize,
    /// `Success` runs within the sample.
    pub successes: usize,
    pub streak: Streak,
    /// Count of status flips (success→failure or vice versa) across adjacent runs in the
    /// sample — high relative to `sample_size` indicates flakiness rather than a steady trend.
    pub flips: usize,
}

impl RoutineReliability {
    /// `successes / sample_size`, or `None` when the sample is empty.
    pub(crate) fn success_rate(&self) -> Option<f64> {
        if self.sample_size == 0 {
            None
        } else {
            Some(self.successes as f64 / self.sample_size as f64)
        }
    }

    /// `true` when this routine's flip rate crosses the flaky threshold.
    pub(crate) fn is_flaky(&self) -> bool {
        if self.sample_size < FLAKY_MIN_SAMPLE {
            return false;
        }
        let pairs = self.sample_size - 1;
        pairs > 0 && (self.flips as f64) / (pairs as f64) >= FLAKY_FLIP_RATIO
    }
}

/// Fleet-wide reliability summary, aggregated across every routine's sample.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct FleetReliability {
    pub sample_size: usize,
    pub successes: usize,
    /// Routines with an active (>= 1 run) failure streak.
    pub failing_count: usize,
    pub flaky_count: usize,
}

impl FleetReliability {
    /// `successes / sample_size`, or `None` when nothing has been sampled yet.
    pub(crate) fn success_rate(&self) -> Option<f64> {
        if self.sample_size == 0 {
            None
        } else {
            Some(self.successes as f64 / self.sample_size as f64)
        }
    }
}

/// Per-routine accumulator while bucketing the fleet-wide run list.
struct RoutineAcc {
    title: String,
    /// Newest-first, capped at `SAMPLE_LEN`.
    statuses: Vec<RunStatus>,
}

/// Buckets a fleet-wide, newest-first run list by routine, keeping only `Success`/`Failed` runs
/// and capping each bucket at `SAMPLE_LEN`, newest-first.
fn bucket_finished_runs(runs: &[FleetRunSummary]) -> HashMap<String, RoutineAcc> {
    let mut by_routine: HashMap<String, RoutineAcc> = HashMap::new();
    for run in runs {
        if !matches!(run.status, RunStatus::Success | RunStatus::Failed) {
            continue;
        }
        let acc = by_routine
            .entry(run.routine_id.clone())
            .or_insert_with(|| RoutineAcc {
                title: run.routine_title.clone(),
                statuses: Vec::new(),
            });
        if acc.statuses.len() < SAMPLE_LEN {
            acc.statuses.push(run.status);
        }
    }
    by_routine
}

/// The active streak at the head (newest-first) of a finished-run sample.
fn compute_streak(statuses: &[RunStatus]) -> Streak {
    let Some(&newest) = statuses.first() else {
        return Streak::None;
    };
    let n = statuses.iter().take_while(|&&s| s == newest).count();
    if newest == RunStatus::Success {
        Streak::Success(n)
    } else {
        Streak::Failure(n)
    }
}

/// Count of adjacent-pair status changes in a finished-run sample.
fn count_flips(statuses: &[RunStatus]) -> usize {
    statuses
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count()
}

/// The length of an active failure streak, or 0 for a success streak or no sample.
fn failure_streak_len(streak: Streak) -> usize {
    match streak {
        Streak::Failure(n) => n,
        Streak::Success(_) | Streak::None => 0,
    }
}

/// Computes every routine's reliability metrics from a fleet-wide run list, ranked
/// worst-first: an active failure streak outranks everything else (longer streak first),
/// then lowest success rate, then title for a stable tie-break. Routines with no finished
/// run in the sample are omitted — there is nothing to rank.
pub(crate) fn compute_reliability(runs: &[FleetRunSummary]) -> Vec<RoutineReliability> {
    let mut items: Vec<RoutineReliability> = bucket_finished_runs(runs)
        .into_iter()
        .map(|(routine_id, acc)| RoutineReliability {
            sample_size: acc.statuses.len(),
            successes: acc
                .statuses
                .iter()
                .filter(|&&s| s == RunStatus::Success)
                .count(),
            streak: compute_streak(&acc.statuses),
            flips: count_flips(&acc.statuses),
            routine_title: acc.title,
            routine_id,
        })
        .collect();

    items.sort_by(|a, b| {
        failure_streak_len(b.streak)
            .cmp(&failure_streak_len(a.streak))
            .then_with(|| {
                let a_rate = a.success_rate().unwrap_or(1.0);
                let b_rate = b.success_rate().unwrap_or(1.0);
                a_rate.partial_cmp(&b_rate).unwrap_or(Ordering::Equal)
            })
            .then_with(|| a.routine_title.cmp(&b.routine_title))
    });
    items
}

/// Aggregates per-routine metrics into a fleet-wide summary for the page's stat tiles.
pub(crate) fn fleet_summary(items: &[RoutineReliability]) -> FleetReliability {
    FleetReliability {
        sample_size: items.iter().map(|r| r.sample_size).sum(),
        successes: items.iter().map(|r| r.successes).sum(),
        failing_count: items
            .iter()
            .filter(|r| failure_streak_len(r.streak) > 0)
            .count(),
        flaky_count: items.iter().filter(|r| r.is_flaky()).count(),
    }
}

/// CSS class for a routine's streak badge (reuses the generic `run-status` pill classes).
pub(crate) fn streak_class(streak: Streak) -> &'static str {
    match streak {
        Streak::Success(_) => "run-status success",
        Streak::Failure(_) => "run-status failed",
        Streak::None => "run-status unknown",
    }
}

/// Display label for a routine's streak badge.
pub(crate) fn streak_label(streak: Streak) -> String {
    match streak {
        Streak::Success(n) => format!("{n} OK"),
        Streak::Failure(n) => format!("{n} FAILING"),
        Streak::None => "—".to_string(),
    }
}

/// CSS class for a success-rate badge (reuses the generic `run-status` pill classes).
pub(crate) fn rate_class(rate: Option<f64>) -> &'static str {
    match rate {
        None => "run-status unknown",
        Some(r) if r >= 0.9 => "run-status success",
        Some(r) if r >= 0.7 => "run-status running",
        Some(_) => "run-status failed",
    }
}

/// Display label for a success-rate badge.
pub(crate) fn rate_label(rate: Option<f64>) -> String {
    match rate {
        None => "—".to_string(),
        Some(r) => format!("{:.0}%", r * 100.0),
    }
}

#[cfg(test)]
#[path = "reliability_stats_tests.rs"]
mod reliability_stats_tests;

//! Inline run-history sparkline for the Routines table: a compact strip of ticks giving an
//! at-a-glance pass/fail trend per routine, mirroring the "pipeline graph" pattern common to CI
//! dashboards (GitHub Actions' per-workflow run history, GitLab's pipeline mini-graph) without
//! navigating to the routine's full HISTORY page.

use std::collections::HashMap;

use yew::prelude::*;

use crate::reltime;

use super::history::run_status_label;
use super::model::{FleetRunSummary, RunStatus};

/// Fleet-wide run count fetched to build every routine's sparkline. A global cap (not
/// per-routine — the backing `GET /routines/runs` endpoint truncates the newest-first merged
/// list to this many total), large enough that an active fleet's routines each keep a several-run
/// trend without an unbounded payload.
pub(crate) const RUN_HISTORY_FETCH_LIMIT: usize = 300;

/// Max ticks rendered per routine, oldest to newest (left to right).
const SPARKLINE_LEN: usize = 10;

/// Buckets a fleet-wide, newest-first run list by routine, keeping each routine's most recent
/// [`SPARKLINE_LEN`] runs in chronological (oldest-first) order for left-to-right rendering.
pub(crate) fn group_recent_runs(
    runs: Vec<FleetRunSummary>,
) -> HashMap<String, Vec<FleetRunSummary>> {
    let mut by_routine: HashMap<String, Vec<FleetRunSummary>> = HashMap::new();
    for run in runs {
        let bucket = by_routine.entry(run.routine_id.clone()).or_default();
        if bucket.len() < SPARKLINE_LEN {
            bucket.push(run);
        }
    }
    for bucket in by_routine.values_mut() {
        bucket.reverse();
    }
    by_routine
}

/// CSS class for one sparkline tick, colour-coded by run outcome (mirrors `run_status_class`'s
/// palette, but as a small tick rather than a text badge).
fn spark_tick_class(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "spark-tick running",
        RunStatus::Success => "spark-tick success",
        RunStatus::Failed => "spark-tick failed",
        RunStatus::Unknown => "spark-tick unknown",
    }
}

#[derive(Properties, PartialEq, Eq)]
pub struct RunHistorySparklineProps {
    /// This routine's recent runs, oldest to newest (as produced by [`group_recent_runs`]).
    pub runs: Vec<FleetRunSummary>,
}

#[function_component(RunHistorySparkline)]
pub fn run_history_sparkline(props: &RunHistorySparklineProps) -> Html {
    if props.runs.is_empty() {
        return html! { <span class="spark-empty muted">{"—"}</span> };
    }
    html! {
        <div class="spark" role="img" aria-label={format!("Last {} runs", props.runs.len())}>
            { for props.runs.iter().map(|run| {
                let title = format!("{} · {}", run_status_label(run.status), reltime(run.started_at));
                html! { <span class={spark_tick_class(run.status)} {title}></span> }
            }) }
        </div>
    }
}

#[cfg(test)]
#[path = "sparkline_tests.rs"]
mod sparkline_tests;

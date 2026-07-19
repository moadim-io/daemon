//! Host-side unit tests for the pure run-history grouping/formatting helpers in [`super`]. No
//! DOM/wasm dependency (mirrors the `history.rs` test conventions) — `RunHistorySparkline`'s
//! `html!` output isn't covered here for the same reason no other component in this module is.

use super::*;

fn run(routine_id: &str, started_at: u64, status: RunStatus) -> FleetRunSummary {
    FleetRunSummary {
        routine_id: routine_id.to_string(),
        routine_title: routine_id.to_string(),
        workbench: format!("{routine_id}-{started_at}"),
        started_at,
        finished_at: Some(started_at + 1),
        status,
        exit_code: None,
    }
}

#[test]
fn group_recent_runs_empty_input_is_empty_map() {
    assert!(group_recent_runs(Vec::new()).is_empty());
}

#[test]
fn group_recent_runs_buckets_by_routine_id() {
    let runs = vec![
        run("a", 300, RunStatus::Success),
        run("b", 200, RunStatus::Failed),
        run("a", 100, RunStatus::Failed),
    ];
    let by_routine = group_recent_runs(runs);
    assert_eq!(by_routine.len(), 2);
    assert_eq!(by_routine["a"].len(), 2);
    assert_eq!(by_routine["b"].len(), 1);
}

#[test]
fn group_recent_runs_reverses_newest_first_input_to_oldest_first() {
    // Fleet-wide list arrives newest-first; the sparkline renders oldest-to-newest left-to-right.
    let runs = vec![
        run("a", 300, RunStatus::Success),
        run("a", 100, RunStatus::Failed),
    ];
    let by_routine = group_recent_runs(runs);
    let a = &by_routine["a"];
    assert_eq!(a[0].started_at, 100);
    assert_eq!(a[1].started_at, 300);
}

#[test]
fn group_recent_runs_caps_at_sparkline_len_per_routine() {
    let runs: Vec<FleetRunSummary> = (0..SPARKLINE_LEN + 5)
        .map(|i| run("a", i as u64, RunStatus::Success))
        .collect();
    let by_routine = group_recent_runs(runs);
    assert_eq!(by_routine["a"].len(), SPARKLINE_LEN);
}

#[test]
fn group_recent_runs_keeps_the_newest_runs_when_capping() {
    // Input is newest-first; capping must keep the front (newest) slice, not the tail.
    let runs: Vec<FleetRunSummary> = (0..SPARKLINE_LEN + 3)
        .rev()
        .map(|i| run("a", i as u64, RunStatus::Success))
        .collect();
    let by_routine = group_recent_runs(runs);
    let a = &by_routine["a"];
    assert_eq!(a.len(), SPARKLINE_LEN);
    // Oldest-first after grouping, so the newest run (started_at == SPARKLINE_LEN + 2) is last.
    assert_eq!(a.last().unwrap().started_at, (SPARKLINE_LEN + 2) as u64);
    assert_eq!(a.first().unwrap().started_at, 3);
}

#[test]
fn spark_tick_class_covers_every_variant() {
    assert_eq!(spark_tick_class(RunStatus::Running), "spark-tick running");
    assert_eq!(spark_tick_class(RunStatus::Success), "spark-tick success");
    assert_eq!(spark_tick_class(RunStatus::Failed), "spark-tick failed");
    assert_eq!(spark_tick_class(RunStatus::Unknown), "spark-tick unknown");
}

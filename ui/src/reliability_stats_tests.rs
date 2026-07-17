//! Host-side unit tests for the pure reliability-metrics math in [`super`]. No DOM/wasm
//! dependency (mirrors the `sparkline_tests.rs` test conventions).

use super::*;

/// Builds a `FleetRunSummary` with just the fields the reliability math reads.
/// `started_at` doubles as an ordering key: callers pass runs newest-first, exactly like the
/// real `GET /routines/runs` response.
fn run(routine_id: &str, title: &str, started_at: u64, status: RunStatus) -> FleetRunSummary {
    FleetRunSummary {
        routine_id: routine_id.to_string(),
        routine_title: title.to_string(),
        workbench: format!("{routine_id}-{started_at}"),
        started_at,
        finished_at: Some(started_at + 1),
        status,
        exit_code: None,
    }
}

// ─── compute_reliability / bucketing ───────────────────────────────────────────

#[test]
fn compute_reliability_empty_input_is_empty() {
    assert_eq!(compute_reliability(&[]), Vec::new());
}

#[test]
fn compute_reliability_excludes_running_and_unknown_runs() {
    let runs = vec![
        run("a", "Alpha", 3, RunStatus::Running),
        run("a", "Alpha", 2, RunStatus::Success),
        run("a", "Alpha", 1, RunStatus::Unknown),
    ];
    let items = compute_reliability(&runs);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].sample_size, 1);
    assert_eq!(items[0].successes, 1);
}

#[test]
fn compute_reliability_omits_routines_with_no_finished_run() {
    let runs = vec![run("a", "Alpha", 1, RunStatus::Running)];
    assert!(compute_reliability(&runs).is_empty());
}

#[test]
fn compute_reliability_caps_sample_at_sample_len() {
    let runs: Vec<FleetRunSummary> = (0..SAMPLE_LEN + 10)
        .map(|i| run("a", "Alpha", i as u64, RunStatus::Success))
        .collect();
    let items = compute_reliability(&runs);
    assert_eq!(items[0].sample_size, SAMPLE_LEN);
}

#[test]
fn compute_reliability_keeps_newest_runs_when_capping() {
    // 25 runs, oldest is a lone failure past the SAMPLE_LEN=20 cutoff; the capped sample should
    // never see it, so the streak reads as a full-success run rather than being cut short.
    let mut runs: Vec<FleetRunSummary> = (0..SAMPLE_LEN + 5)
        .map(|i| {
            run(
                "a",
                "Alpha",
                (SAMPLE_LEN + 5 - i) as u64,
                RunStatus::Success,
            )
        })
        .collect();
    runs.push(run("a", "Alpha", 0, RunStatus::Failed));
    let items = compute_reliability(&runs);
    assert_eq!(items[0].sample_size, SAMPLE_LEN);
    assert_eq!(items[0].successes, SAMPLE_LEN);
    assert_eq!(items[0].streak, Streak::Success(SAMPLE_LEN));
}

// ─── streak ─────────────────────────────────────────────────────────────────

#[test]
fn streak_all_success() {
    let runs = vec![
        run("a", "Alpha", 3, RunStatus::Success),
        run("a", "Alpha", 2, RunStatus::Success),
        run("a", "Alpha", 1, RunStatus::Success),
    ];
    assert_eq!(compute_reliability(&runs)[0].streak, Streak::Success(3));
}

#[test]
fn streak_stops_at_first_status_change_newest_first() {
    // Newest-first input: 2 recent failures, then older successes — streak should read as
    // Failure(2), not be thrown off by the older, unrelated successes.
    let runs = vec![
        run("a", "Alpha", 4, RunStatus::Failed),
        run("a", "Alpha", 3, RunStatus::Failed),
        run("a", "Alpha", 2, RunStatus::Success),
        run("a", "Alpha", 1, RunStatus::Success),
    ];
    assert_eq!(compute_reliability(&runs)[0].streak, Streak::Failure(2));
}

#[test]
fn streak_none_when_sample_empty() {
    assert_eq!(compute_streak(&[]), Streak::None);
}

// ─── flips / flaky ──────────────────────────────────────────────────────────

#[test]
fn count_flips_counts_adjacent_status_changes() {
    assert_eq!(
        count_flips(&[
            RunStatus::Success,
            RunStatus::Failed,
            RunStatus::Failed,
            RunStatus::Success,
        ]),
        2
    );
}

#[test]
fn count_flips_zero_for_uniform_sample() {
    assert_eq!(count_flips(&[RunStatus::Success, RunStatus::Success]), 0);
}

#[test]
fn is_flaky_false_below_minimum_sample() {
    // 4 runs alternating every time (3 flips out of 3 pairs = 100%) but under FLAKY_MIN_SAMPLE.
    let runs = vec![
        run("a", "Alpha", 4, RunStatus::Success),
        run("a", "Alpha", 3, RunStatus::Failed),
        run("a", "Alpha", 2, RunStatus::Success),
        run("a", "Alpha", 1, RunStatus::Failed),
    ];
    assert!(!compute_reliability(&runs)[0].is_flaky());
}

#[test]
fn is_flaky_true_at_or_above_ratio_with_enough_sample() {
    // 6 runs alternating throughout: 5 flips / 5 pairs = 100% >= 40% ratio, sample_size 6 >= 5.
    let runs = vec![
        run("a", "Alpha", 6, RunStatus::Success),
        run("a", "Alpha", 5, RunStatus::Failed),
        run("a", "Alpha", 4, RunStatus::Success),
        run("a", "Alpha", 3, RunStatus::Failed),
        run("a", "Alpha", 2, RunStatus::Success),
        run("a", "Alpha", 1, RunStatus::Failed),
    ];
    assert!(compute_reliability(&runs)[0].is_flaky());
}

#[test]
fn is_flaky_false_for_steady_run_with_enough_sample() {
    let runs: Vec<FleetRunSummary> = (0..6)
        .map(|i| run("a", "Alpha", i as u64, RunStatus::Success))
        .collect();
    assert!(!compute_reliability(&runs)[0].is_flaky());
}

// ─── success_rate ───────────────────────────────────────────────────────────

#[test]
fn success_rate_none_when_sample_empty() {
    let r = RoutineReliability {
        routine_id: "a".into(),
        routine_title: "Alpha".into(),
        sample_size: 0,
        successes: 0,
        streak: Streak::None,
        flips: 0,
    };
    assert_eq!(r.success_rate(), None);
}

#[test]
fn success_rate_computed_from_successes_over_sample_size() {
    let runs = vec![
        run("a", "Alpha", 3, RunStatus::Success),
        run("a", "Alpha", 2, RunStatus::Success),
        run("a", "Alpha", 1, RunStatus::Failed),
    ];
    let rate = compute_reliability(&runs)[0].success_rate().unwrap();
    assert!((rate - (2.0 / 3.0)).abs() < f64::EPSILON);
}

// ─── ranking order ──────────────────────────────────────────────────────────

#[test]
fn ranks_active_failure_streak_before_low_success_rate() {
    // Beta has a lower overall success rate but no *active* failure streak (its worst run is
    // buried in the middle); Alpha is currently failing right now. Alpha should rank first —
    // an active incident outranks a merely-mediocre history.
    let runs = vec![
        run("alpha", "Alpha", 2, RunStatus::Failed),
        run("alpha", "Alpha", 1, RunStatus::Success),
        run("beta", "Beta", 4, RunStatus::Success),
        run("beta", "Beta", 3, RunStatus::Failed),
        run("beta", "Beta", 2, RunStatus::Failed),
        run("beta", "Beta", 1, RunStatus::Success),
    ];
    let items = compute_reliability(&runs);
    assert_eq!(items[0].routine_id, "alpha");
    assert_eq!(items[1].routine_id, "beta");
}

#[test]
fn ranks_longer_failure_streak_first() {
    let runs = vec![
        run("short", "Short", 1, RunStatus::Failed),
        run("long", "Long", 2, RunStatus::Failed),
        run("long", "Long", 1, RunStatus::Failed),
    ];
    let items = compute_reliability(&runs);
    assert_eq!(items[0].routine_id, "long");
    assert_eq!(items[1].routine_id, "short");
}

#[test]
fn ranks_lower_success_rate_first_when_no_active_failure_streak() {
    let runs = vec![
        run("worse", "Worse", 4, RunStatus::Success),
        run("worse", "Worse", 3, RunStatus::Failed),
        run("worse", "Worse", 2, RunStatus::Success),
        run("better", "Better", 2, RunStatus::Success),
        run("better", "Better", 1, RunStatus::Success),
    ];
    let items = compute_reliability(&runs);
    assert_eq!(items[0].routine_id, "worse");
    assert_eq!(items[1].routine_id, "better");
}

#[test]
fn ranks_ties_by_title() {
    let runs = vec![
        run("z", "Zeta", 1, RunStatus::Success),
        run("a", "Alpha", 1, RunStatus::Success),
    ];
    let items = compute_reliability(&runs);
    assert_eq!(items[0].routine_id, "a");
    assert_eq!(items[1].routine_id, "z");
}

// ─── fleet_summary ──────────────────────────────────────────────────────────

#[test]
fn fleet_summary_of_empty_is_zeroed() {
    assert_eq!(fleet_summary(&[]), FleetReliability::default());
}

#[test]
fn fleet_summary_aggregates_across_routines() {
    let runs = vec![
        run("a", "Alpha", 2, RunStatus::Failed),
        run("a", "Alpha", 1, RunStatus::Success),
        run("b", "Beta", 1, RunStatus::Success),
    ];
    let items = compute_reliability(&runs);
    let summary = fleet_summary(&items);
    assert_eq!(summary.sample_size, 3);
    assert_eq!(summary.successes, 2);
    assert_eq!(summary.failing_count, 1);
}

#[test]
fn fleet_reliability_success_rate_none_when_unsampled() {
    assert_eq!(FleetReliability::default().success_rate(), None);
}

#[test]
fn fleet_reliability_success_rate_computed() {
    let summary = FleetReliability {
        sample_size: 4,
        successes: 3,
        failing_count: 0,
        flaky_count: 0,
    };
    assert!((summary.success_rate().unwrap() - 0.75).abs() < f64::EPSILON);
}

// ─── badge class/label helpers ──────────────────────────────────────────────

#[test]
fn streak_class_covers_every_variant() {
    assert_eq!(streak_class(Streak::Success(1)), "run-status success");
    assert_eq!(streak_class(Streak::Failure(1)), "run-status failed");
    assert_eq!(streak_class(Streak::None), "run-status unknown");
}

#[test]
fn streak_label_covers_every_variant() {
    assert_eq!(streak_label(Streak::Success(3)), "3 OK");
    assert_eq!(streak_label(Streak::Failure(2)), "2 FAILING");
    assert_eq!(streak_label(Streak::None), "—");
}

#[test]
fn rate_class_thresholds() {
    assert_eq!(rate_class(None), "run-status unknown");
    assert_eq!(rate_class(Some(1.0)), "run-status success");
    assert_eq!(rate_class(Some(0.9)), "run-status success");
    assert_eq!(rate_class(Some(0.89)), "run-status running");
    assert_eq!(rate_class(Some(0.7)), "run-status running");
    assert_eq!(rate_class(Some(0.69)), "run-status failed");
    assert_eq!(rate_class(Some(0.0)), "run-status failed");
}

#[test]
fn rate_label_formats_percentage_or_dash() {
    assert_eq!(rate_label(None), "—");
    assert_eq!(rate_label(Some(0.5)), "50%");
    assert_eq!(rate_label(Some(1.0)), "100%");
}

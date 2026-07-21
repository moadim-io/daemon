/**
 * Pure math backing the inline run-history sparkline: a compact strip of ticks giving an
 * at-a-glance pass/fail trend per routine (mirrors the CI "pipeline graph" pattern: GitHub
 * Actions' per-workflow run history, GitLab's pipeline mini-graph).
 * Direct port of the pure parts of `ui/src/routines/sparkline.rs (removed)`.
 */
import type { FleetRunSummary } from "../../api/hooks";

/**
 * Fleet-wide run count fetched to build every routine's sparkline. A global cap (not
 * per-routine — the backing endpoint truncates the newest-first merged list to this many total).
 */
export const RUN_HISTORY_FETCH_LIMIT = 300;

/** Max ticks rendered per routine, oldest to newest (left to right). */
export const SPARKLINE_LEN = 10;

/**
 * Buckets a fleet-wide, newest-first run list by routine, keeping each routine's most recent
 * `SPARKLINE_LEN` runs in chronological (oldest-first) order for left-to-right rendering.
 */
export function groupRecentRuns(runs: FleetRunSummary[]): Map<string, FleetRunSummary[]> {
  const byRoutine = new Map<string, FleetRunSummary[]>();
  for (const run of runs) {
    let bucket = byRoutine.get(run.routine_id);
    if (!bucket) {
      bucket = [];
      byRoutine.set(run.routine_id, bucket);
    }
    if (bucket.length < SPARKLINE_LEN) bucket.push(run);
  }
  for (const bucket of byRoutine.values()) bucket.reverse();
  return byRoutine;
}

/** Fill color for one sparkline tick, colour-coded by run outcome. */
export function sparkTickClass(status: FleetRunSummary["status"]): string {
  switch (status) {
    case "running":
      return "spark-tick running";
    case "success":
      return "spark-tick success";
    case "failed":
      return "spark-tick failed";
    default:
      return "spark-tick unknown";
  }
}

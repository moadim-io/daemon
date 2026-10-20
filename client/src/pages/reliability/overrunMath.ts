/**
 * Live overrun detection for the RELIABILITY page: compares every still-running run's elapsed
 * time against its routine's own p95 duration, so a hung run surfaces while it is still running
 * instead of only after it finishes or fails.
 *
 * Best practice (cron monitors — Cronitor's "runs longer than expected" alerts): judge each job
 * against its own history, not a global timeout, and alert on duration before completion.
 */
import type { FleetRunSummary } from "../../api/hooks";
import type { RoutineReliability } from "./reliabilityStats";

/** Timed finished runs needed before a routine's p95 is trusted as a baseline. */
export const MIN_BASELINE_RUNS = 3;

/** Elapsed must exceed p95 by this factor to flag a run. */
export const OVERRUN_FACTOR = 1.5;

/** Runs younger than this are never flagged — a 5 s p95 doesn't make a 10 s run interesting. */
export const MIN_OVERRUN_SECS = 60;

export interface Overrun {
  routineId: string;
  routineTitle: string;
  workbench: string;
  elapsedSecs: number;
  p95Secs: number;
  /** `elapsedSecs / p95Secs`. */
  ratio: number;
}

export interface OverrunReport {
  /** Flagged runs, worst ratio first. */
  overruns: Overrun[];
  /** Running runs in the sample. */
  runningCount: number;
  /** Running runs whose routine lacks a `MIN_BASELINE_RUNS` baseline. */
  noBaselineCount: number;
}

export function computeOverruns(runs: FleetRunSummary[], items: RoutineReliability[], now: number): OverrunReport {
  const baselines = new Map(items.map((i) => [i.routineId, i]));
  const overruns: Overrun[] = [];
  let runningCount = 0;
  let noBaselineCount = 0;
  for (const run of runs) {
    if (run.status !== "running") continue;
    runningCount += 1;
    const base = baselines.get(run.routine_id);
    if (!base || base.p95Secs === null || base.durationsSecs.length < MIN_BASELINE_RUNS) {
      noBaselineCount += 1;
      continue;
    }
    const elapsedSecs = Math.max(0, now - run.started_at);
    // A 0 s p95 (instant routine) still needs MIN_OVERRUN_SECS before it counts.
    const p95Secs = Math.max(base.p95Secs, 1);
    if (elapsedSecs < MIN_OVERRUN_SECS || elapsedSecs <= p95Secs * OVERRUN_FACTOR) continue;
    overruns.push({
      routineId: run.routine_id,
      routineTitle: run.routine_title,
      workbench: run.workbench,
      elapsedSecs,
      p95Secs: base.p95Secs,
      ratio: elapsedSecs / p95Secs,
    });
  }
  overruns.sort((a, b) => b.ratio - a.ratio);
  return { overruns, runningCount, noBaselineCount };
}

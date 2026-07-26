/**
 * Pure math backing the per-routine run-duration chart: a horizontal bar per run, sized
 * relative to the slowest run in view, so a slow outlier or a creeping trend is visible at a
 * glance (mirrors GitHub Actions' relative-duration run list / CircleCI Insights duration chart).
 */
import type { RunSummary } from "../../api/hooks";

/** Bars for runs still in flight (or otherwise missing a finish time) render at this floor width. */
export const MIN_BAR_PCT = 4;

export interface DurationBar {
  workbench: string;
  startedAt: number;
  status: RunSummary["status"];
  exitCode: number | null | undefined;
  /** Wall-clock seconds, or `null` when the run hasn't finished. */
  durationSecs: number | null;
  /** Bar width as a percentage of the slowest finished run in the set, floored at `MIN_BAR_PCT`. */
  widthPct: number;
}

/** Builds one bar per run, preserving input order. */
export function buildDurationBars(runs: RunSummary[]): DurationBar[] {
  const durations = runs.map((run) =>
    run.finished_at != null ? Math.max(0, run.finished_at - run.started_at) : null,
  );
  const maxSecs = Math.max(1, ...durations.filter((d): d is number => d !== null));

  return runs.map((run, i) => {
    const durationSecs = durations[i] ?? null;
    return {
      workbench: run.workbench,
      startedAt: run.started_at,
      status: run.status,
      exitCode: run.exit_code,
      durationSecs,
      widthPct: durationSecs === null ? MIN_BAR_PCT : Math.max(MIN_BAR_PCT, (durationSecs / maxSecs) * 100),
    };
  });
}

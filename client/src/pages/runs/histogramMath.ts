import type { FleetRunSummary } from "../../api/hooks";

type RunStatus = FleetRunSummary["status"];

/** One time bucket of the activity histogram: `[from, to)` in unix seconds. */
export interface HistogramBucket {
  from: number;
  to: number;
  counts: Record<RunStatus, number>;
  total: number;
}

export interface RunsHistogram {
  buckets: HistogramBucket[];
  /** Width of every bucket, in seconds. */
  step: number;
  /** Largest bucket total — the bar-height denominator (0 when empty). */
  max: number;
}

/** Target bar count; the real count lands at or just above this after snapping to a round step. */
export const TARGET_BUCKETS = 24;

/** Round bucket widths to snap to, so bars line up with wall-clock minutes/hours/days. */
const STEPS = [60, 300, 900, 1_800, 3_600, 3 * 3_600, 6 * 3_600, 12 * 3_600, 86_400, 7 * 86_400];

/** Smallest round step that covers `span` seconds in at most `TARGET_BUCKETS` bars. */
export function pickStep(span: number): number {
  // ponytail: spans over ~24 weeks just get more weekly bars; fetched runs cap at 1 000 anyway.
  return STEPS.find((s) => s * TARGET_BUCKETS >= span) ?? STEPS[STEPS.length - 1]!;
}

/**
 * Buckets `runs` by `started_at` into a status-stacked histogram spanning `[from, now]`.
 * `from` defaults to the oldest run's start. Bucket edges are aligned to multiples of the step
 * (UTC), so the same run lands in the same bar across refreshes.
 */
export function buildHistogram(runs: FleetRunSummary[], nowSecs: number, from?: number): RunsHistogram {
  const start = from ?? Math.min(...runs.map((r) => r.started_at));
  if (!Number.isFinite(start) || start > nowSecs) return { buckets: [], step: 0, max: 0 };
  const step = pickStep(nowSecs - start);
  const first = Math.floor(start / step) * step;
  const count = Math.floor((nowSecs - first) / step) + 1;
  const buckets: HistogramBucket[] = Array.from({ length: count }, (_, i) => ({
    from: first + i * step,
    to: first + (i + 1) * step,
    counts: { running: 0, success: 0, failed: 0, unknown: 0 },
    total: 0,
  }));
  for (const run of runs) {
    const b = buckets[Math.floor((run.started_at - first) / step)];
    if (b === undefined) continue;
    b.counts[run.status] += 1;
    b.total += 1;
  }
  return { buckets, step, max: Math.max(0, ...buckets.map((b) => b.total)) };
}

/** Runs whose start falls inside `[range.from, range.to)`. */
export function runsInRange(runs: FleetRunSummary[], range: { from: number; to: number }): FleetRunSummary[] {
  return runs.filter((r) => r.started_at >= range.from && r.started_at < range.to);
}

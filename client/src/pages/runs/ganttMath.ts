import type { FleetRunSummary } from "../../api/hooks";

/** One run drawn as a bar spanning `[start, end]` (unix seconds), clipped to the window. */
export interface GanttBar {
  run: FleetRunSummary;
  start: number;
  end: number;
}

/** One routine's swimlane. */
export interface GanttLane {
  routineId: string;
  title: string;
  bars: GanttBar[];
}

/** A constant-concurrency span `[from, to)` of the concurrency strip. */
export interface ConcurrencySpan {
  from: number;
  to: number;
  count: number;
}

export interface Gantt {
  from: number;
  to: number;
  lanes: GanttLane[];
  concurrency: ConcurrencySpan[];
  /** Most runs executing at the same instant inside the window. */
  peak: number;
}

/**
 * When a run stopped executing: its finish time, `now` while still running, or its start when
 * it never recorded a finish (unknown status) — drawn as a zero-width tick, never counted as
 * concurrent.
 */
export function runEnd(run: FleetRunSummary, nowSecs: number): number {
  if (run.finished_at != null) return Math.max(run.started_at, run.finished_at);
  return run.status === "running" ? Math.max(run.started_at, nowSecs) : run.started_at;
}

/**
 * Sweeps start/end events into constant-count spans over `[from, to]`. Ends sort before starts
 * at the same instant, so back-to-back runs don't read as overlapping.
 */
export function concurrencySpans(bars: { start: number; end: number }[], from: number, to: number): ConcurrencySpan[] {
  const events: [number, number][] = [];
  for (const b of bars) {
    if (b.end <= b.start) continue;
    events.push([b.start, 1], [b.end, -1]);
  }
  events.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const spans: ConcurrencySpan[] = [];
  let count = 0;
  let cursor = from;
  for (const [t, delta] of events) {
    if (t > cursor) {
      spans.push({ from: cursor, to: t, count });
      cursor = t;
    }
    count += delta;
  }
  if (to > cursor) spans.push({ from: cursor, to, count });
  return spans;
}

/**
 * Lays `runs` out as per-routine swimlanes over `[from, now]` (`from` defaults to the oldest
 * run's start) and computes the fleet concurrency strip. Runs entirely outside the window are
 * dropped; the rest are clipped to it. Lanes sort by title.
 */
export function buildGantt(runs: FleetRunSummary[], nowSecs: number, from?: number): Gantt {
  const start = from ?? Math.min(...runs.map((r) => r.started_at));
  if (!Number.isFinite(start) || start >= nowSecs) return { from: 0, to: 0, lanes: [], concurrency: [], peak: 0 };
  const lanes = new Map<string, GanttLane>();
  const bars: GanttBar[] = [];
  for (const run of runs) {
    const end = Math.min(runEnd(run, nowSecs), nowSecs);
    if (end < start || run.started_at > nowSecs) continue;
    const bar = { run, start: Math.max(run.started_at, start), end };
    bars.push(bar);
    let lane = lanes.get(run.routine_id);
    if (lane === undefined) {
      lane = { routineId: run.routine_id, title: run.routine_title, bars: [] };
      lanes.set(run.routine_id, lane);
    }
    lane.bars.push(bar);
  }
  const concurrency = concurrencySpans(bars, start, nowSecs);
  return {
    from: start,
    to: nowSecs,
    lanes: [...lanes.values()].sort((a, b) => a.title.localeCompare(b.title)),
    concurrency,
    peak: Math.max(0, ...concurrency.map((s) => s.count)),
  };
}

/** Percentage offset of `t` inside `[from, to]`. */
export function pct(t: number, from: number, to: number): number {
  return ((t - from) / (to - from)) * 100;
}

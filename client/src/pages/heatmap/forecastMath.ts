/**
 * Capacity forecast: projects every enabled routine's fires over the next `horizon` seconds,
 * sizes each projected run by that routine's own median historical duration, and sweeps them into
 * a concurrency strip compared against the daemon's `max_concurrent_runs` cap — the
 * forward-looking counterpart to the Runs page Gantt's past concurrency strip.
 */
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { parseSchedule, scheduleList } from "../../lib/schedule";
import { concurrencySpans, type ConcurrencySpan } from "../runs/ganttMath";

/** Forecast window: the next 24 hours. */
export const FORECAST_HORIZON_SECS = 24 * 3_600;
/** Duration assumed for a routine with no finished run history. */
export const DEFAULT_ESTIMATE_SECS = 10 * 60;
/** Bounds fire iteration per schedule so a per-minute cron can't stall the page (1440/day). */
const MAX_FIRES_PER_SCHEDULE = 2_000;

/** One projected run `[start, end)` (unix seconds). */
export interface ForecastBar {
  start: number;
  end: number;
  /** Already running now rather than a future fire. */
  running: boolean;
}

export interface ForecastLane {
  routineId: string;
  title: string;
  /** Seconds each projected run is assumed to take. */
  estimateSecs: number;
  /** `true` when `estimateSecs` is the default because the routine has no finished history. */
  estimated: boolean;
  bars: ForecastBar[];
}

/** A maximal stretch where projected concurrency exceeds the cap, with who collides in it. */
export interface OverCapWindow {
  from: number;
  to: number;
  peak: number;
  titles: string[];
}

export interface Forecast {
  from: number;
  to: number;
  lanes: ForecastLane[];
  concurrency: ConcurrencySpan[];
  peak: number;
  overCap: OverCapWindow[];
}

/** Median finished-run duration (seconds) per routine id. */
export function medianDurations(runs: FleetRunSummary[]): Map<string, number> {
  const byRoutine = new Map<string, number[]>();
  for (const run of runs) {
    if (run.finished_at == null || run.status === "running") continue;
    const list = byRoutine.get(run.routine_id) ?? [];
    list.push(Math.max(0, run.finished_at - run.started_at));
    byRoutine.set(run.routine_id, list);
  }
  const out = new Map<string, number>();
  for (const [id, list] of byRoutine) {
    list.sort((a, b) => a - b);
    out.set(id, list[Math.floor((list.length - 1) / 2)] ?? 0);
  }
  return out;
}

/** Fire times (unix seconds) of `routine` in `(now, to)`, honoring snooze and `skip_runs`. */
export function projectedFires(routine: RoutineResponse, nowSecs: number, to: number): number[] {
  const fires = new Set<number>();
  for (const schedule of scheduleList(routine)) {
    const cron = parseSchedule(schedule, new Date(nowSecs * 1000));
    for (let i = 0; cron !== undefined && i < MAX_FIRES_PER_SCHEDULE && cron.hasNext(); i++) {
      const t = Math.floor(cron.next().toDate().getTime() / 1000);
      if (t >= to) break;
      fires.add(t);
    }
  }
  const snoozedUntil = routine.snoozed_until ?? 0;
  return [...fires]
    .sort((a, b) => a - b)
    .filter((t) => t >= snoozedUntil)
    .slice(Math.max(0, routine.skip_runs ?? 0));
}

/**
 * Builds the forecast over `[now, now + horizon]`. Disabled and dormant (no-machine) routines are
 * skipped; runs currently executing occupy a slot until their expected finish.
 */
export function buildForecast(
  routines: RoutineResponse[],
  runs: FleetRunSummary[],
  nowSecs: number,
  cap: number,
  horizon = FORECAST_HORIZON_SECS,
): Forecast {
  const to = nowSecs + horizon;
  const medians = medianDurations(runs);
  const running = runs.filter((r) => r.status === "running");
  const lanes: ForecastLane[] = [];
  for (const routine of routines) {
    if (!routine.enabled || (routine.machines ?? []).every((m) => m.trim() === "")) continue;
    const median = medians.get(routine.id);
    // ponytail: a 0s median (instant runs) would never overlap anything; floor at one minute.
    const estimateSecs = Math.max(60, median ?? DEFAULT_ESTIMATE_SECS);
    const bars: ForecastBar[] = running
      .filter((r) => r.routine_id === routine.id)
      .map((r) => ({ start: nowSecs, end: Math.min(to, Math.max(nowSecs + 60, r.started_at + estimateSecs)), running: true }));
    for (const t of projectedFires(routine, nowSecs, to)) {
      bars.push({ start: t, end: Math.min(to, t + estimateSecs), running: false });
    }
    if (bars.length > 0) {
      lanes.push({ routineId: routine.id, title: routine.title, estimateSecs, estimated: median === undefined, bars });
    }
  }
  lanes.sort((a, b) => a.title.localeCompare(b.title));
  const concurrency = concurrencySpans(
    lanes.flatMap((l) => l.bars),
    nowSecs,
    to,
  );
  return {
    from: nowSecs,
    to,
    lanes,
    concurrency,
    peak: Math.max(0, ...concurrency.map((s) => s.count)),
    overCap: overCapWindows(concurrency, lanes, cap),
  };
}

/** Merges adjacent over-cap spans (`count > cap`, cap `0` = unbounded) into windows. */
export function overCapWindows(spans: ConcurrencySpan[], lanes: ForecastLane[], cap: number): OverCapWindow[] {
  if (cap <= 0) return [];
  const windows: OverCapWindow[] = [];
  for (const s of spans) {
    if (s.count <= cap) continue;
    const last = windows[windows.length - 1];
    if (last !== undefined && last.to === s.from) {
      last.to = s.to;
      last.peak = Math.max(last.peak, s.count);
    } else {
      windows.push({ from: s.from, to: s.to, peak: s.count, titles: [] });
    }
  }
  for (const w of windows) {
    w.titles = lanes.filter((l) => l.bars.some((b) => b.start < w.to && b.end > w.from)).map((l) => l.title);
  }
  return windows;
}

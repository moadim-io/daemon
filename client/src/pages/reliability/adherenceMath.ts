/**
 * Schedule adherence: replays each routine's cron schedule(s) over a past window and matches every
 * *expected* fire to a run that actually started — the "dead man's switch" view cron monitors
 * (Cronitor, Healthchecks.io) are built around. Success-rate tables only see runs that happened;
 * this counts the ones that never did, plus how late the ones that did started.
 */
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { parseSchedule, scheduleList } from "../../lib/schedule";

/** Audit window: the last 24 hours. */
export const ADHERENCE_WINDOW_SECS = 24 * 3_600;
/** A run starting up to this long after its fire (queueing, startup) still counts as that fire. */
export const GRACE_SECS = 15 * 60;
/** A run starting slightly *before* its fire (clock skew) still counts. */
const EARLY_SECS = 60;
/** Bounds fire iteration per schedule so a per-minute cron can't stall the page (1440/day). */
const MAX_FIRES_PER_SCHEDULE = 2_000;

export interface AdherenceRow {
  routineId: string;
  title: string;
  expected: number;
  ran: number;
  missed: number;
  /** Median seconds between a fire and its matched run's start; `null` when nothing ran. */
  medianLagSecs: number | null;
  /** Most recent expected fire with no matching run. */
  lastMissedAt: number | null;
}

export interface Adherence {
  /** Earliest audited instant (unix seconds). */
  from: number;
  /** Fires after this are still inside their grace period and not judged yet. */
  to: number;
  /** `true` when the fetched run sample is full, so the window was shrunk to its oldest run. */
  truncated: boolean;
  /** Routines with at least one expected fire, most missed first. */
  rows: AdherenceRow[];
  expected: number;
  ran: number;
  missed: number;
  medianLagSecs: number | null;
}

function median(xs: number[]): number | null {
  if (xs.length === 0) return null;
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.floor((s.length - 1) / 2)] ?? null;
}

/** Fire times (unix seconds) of `routine` in `[from, to]`, merged across schedules, ascending. */
export function pastFires(routine: RoutineResponse, from: number, to: number): number[] {
  const fires = new Set<number>();
  for (const schedule of scheduleList(routine)) {
    // Prime one second past `to` so a fire exactly at `to` is included by `prev()`.
    const cron = parseSchedule(schedule, new Date((to + 1) * 1000));
    for (let i = 0; cron !== undefined && i < MAX_FIRES_PER_SCHEDULE && cron.hasPrev(); i++) {
      const t = Math.floor(cron.prev().toDate().getTime() / 1000);
      if (t < from) break;
      fires.add(t);
    }
  }
  return [...fires].sort((a, b) => a - b);
}

/** Greedily pairs each fire with the earliest unused run starting in `[fire - 60s, fire + grace]`. */
export function matchFires(
  fires: number[],
  starts: number[],
): { lags: number[]; missedAt: number[] } {
  const sorted = [...starts].sort((a, b) => a - b);
  const used = new Set<number>();
  const lags: number[] = [];
  const missedAt: number[] = [];
  for (const fire of fires) {
    const idx = sorted.findIndex((s, i) => !used.has(i) && s >= fire - EARLY_SECS && s <= fire + GRACE_SECS);
    if (idx === -1) {
      missedAt.push(fire);
    } else {
      used.add(idx);
      lags.push(Math.max(0, sorted[idx]! - fire));
    }
  }
  return { lags, missedAt };
}

/**
 * Audits enabled routines assigned to `machine` (when known — runs are only recorded where they
 * execute) over the last day. `fetchLimit` is the run sample cap: a full sample means older runs
 * were cut off, so the window starts at the oldest fetched run instead of reporting false misses.
 */
export function computeAdherence(
  routines: RoutineResponse[],
  runs: FleetRunSummary[],
  machine: string | undefined,
  nowSecs: number,
  fetchLimit: number,
): Adherence {
  const truncated = runs.length >= fetchLimit && runs.length > 0;
  const sampleFloor = truncated ? Math.min(...runs.map((r) => r.started_at)) : -Infinity;
  const from = Math.max(nowSecs - ADHERENCE_WINDOW_SECS, sampleFloor);
  const to = nowSecs - GRACE_SECS;
  const startsByRoutine = new Map<string, number[]>();
  for (const run of runs) {
    const list = startsByRoutine.get(run.routine_id) ?? [];
    list.push(run.started_at);
    startsByRoutine.set(run.routine_id, list);
  }
  const rows: AdherenceRow[] = [];
  const allLags: number[] = [];
  for (const routine of routines) {
    const machines = (routine.machines ?? []).filter((m) => m.trim() !== "");
    if (!routine.enabled || machines.length === 0) continue;
    if (machine !== undefined && !machines.includes(machine)) continue;
    // Fires before creation never existed; fires before a current snooze were skipped on purpose.
    const start = Math.max(from, routine.created_at, routine.snoozed_until ?? 0);
    const fires = pastFires(routine, start, to);
    if (fires.length === 0) continue;
    const { lags, missedAt } = matchFires(fires, startsByRoutine.get(routine.id) ?? []);
    allLags.push(...lags);
    rows.push({
      routineId: routine.id,
      title: routine.title,
      expected: fires.length,
      ran: lags.length,
      missed: missedAt.length,
      medianLagSecs: median(lags),
      lastMissedAt: missedAt[missedAt.length - 1] ?? null,
    });
  }
  rows.sort((a, b) => b.missed - a.missed || a.title.localeCompare(b.title));
  const sum = (k: "expected" | "ran" | "missed") => rows.reduce((n, r) => n + r[k], 0);
  return {
    from,
    to,
    truncated,
    rows,
    expected: sum("expected"),
    ran: sum("ran"),
    missed: sum("missed"),
    medianLagSecs: median(allLags),
  };
}

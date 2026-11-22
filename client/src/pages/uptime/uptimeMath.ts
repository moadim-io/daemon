/**
 * Pure math for the UPTIME page: buckets the fleet's runs into calendar days per routine, so an
 * operator sees *which days* a routine broke over the last week, month or quarter.
 *
 * Best practice (Atlassian Statuspage, Crontap, Hyperping uptime bars; Airflow's Grid view): one
 * colour-coded bar per day, a ratio over a fixed window, and a click-through to the runs behind it.
 */
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";

export const RANGE_OPTIONS = [7, 30, 90] as const;
export type UptimeRange = (typeof RANGE_OPTIONS)[number];
export const DEFAULT_RANGE: UptimeRange = 30;

/** `?days=` value, or the default when missing or not one of {@link RANGE_OPTIONS}. */
export function rangeFromParam(value: string | null): UptimeRange {
  return RANGE_OPTIONS.find((r) => String(r) === value) ?? DEFAULT_RANGE;
}

/** `ok` = every finished run succeeded, `partial` = mixed, `down` = every one failed, `none` = no finished run. */
export type DayState = "ok" | "partial" | "down" | "none";

export interface DayCell {
  /** Local midnight, epoch seconds. */
  from: number;
  /** Next local midnight, epoch seconds (exclusive). */
  to: number;
  success: number;
  failed: number;
  /** A run started this day is still live. */
  running: boolean;
  state: DayState;
}

export interface UptimeRow {
  routineId: string;
  title: string;
  days: DayCell[];
  success: number;
  failed: number;
  /** Successes over finished runs in the range; `null` with no finished run. */
  uptime: number | null;
}

export interface UptimeReport {
  /** Lowest uptime first; routines without a finished run last. */
  rows: UptimeRow[];
  /** Every routine's runs folded into one row. */
  fleet: UptimeRow;
  /** Fleet days with at least one failed run. */
  daysWithFailures: number;
  /** Routines at 100% uptime over the range. */
  perfect: number;
  /** The capped run sample does not reach back to the start of the range. */
  partialSample: boolean;
}

/** Local-midnight epoch seconds of the `days` calendar days ending today, oldest first, plus tomorrow's. */
export function dayBounds(nowSecs: number, days: number): number[] {
  const today = new Date(nowSecs * 1000);
  // `new Date(y, m, d)` normalises overflow and follows DST, unlike adding 86 400 s.
  return Array.from(
    { length: days + 1 },
    (_, i) => new Date(today.getFullYear(), today.getMonth(), today.getDate() - days + 1 + i).getTime() / 1000,
  );
}

function stateOf(success: number, failed: number): DayState {
  if (success + failed === 0) return "none";
  if (failed === 0) return "ok";
  return success === 0 ? "down" : "partial";
}

function emptyDays(bounds: number[]): DayCell[] {
  return bounds
    .slice(0, -1)
    .map((from, i) => ({ from, to: bounds[i + 1]!, success: 0, failed: 0, running: false, state: "none" }));
}

function finish(row: UptimeRow): UptimeRow {
  for (const day of row.days) {
    day.state = stateOf(day.success, day.failed);
    row.success += day.success;
    row.failed += day.failed;
  }
  const finished = row.success + row.failed;
  row.uptime = finished === 0 ? null : row.success / finished;
  return row;
}

export function buildUptime(
  runs: FleetRunSummary[],
  routines: RoutineResponse[],
  nowSecs: number,
  days: number,
  fetchLimit: number,
): UptimeReport {
  const bounds = dayBounds(nowSecs, days);
  const start = bounds[0]!;
  const end = bounds[days]!;
  const newRow = (routineId: string, title: string): UptimeRow => ({
    routineId,
    title,
    days: emptyDays(bounds),
    success: 0,
    failed: 0,
    uptime: null,
  });

  const byId = new Map<string, UptimeRow>();
  for (const r of routines) byId.set(r.id, newRow(r.id, r.title));
  const fleet = newRow("", "Fleet");

  for (const run of runs) {
    if (run.started_at < start || run.started_at >= end) continue;
    let row = byId.get(run.routine_id);
    if (!row) {
      // A deleted routine still has runs on disk: keep it, under the title it ran with.
      row = newRow(run.routine_id, run.routine_title);
      byId.set(run.routine_id, row);
    }
    // ponytail: linear scan over ≤ 90 days per run; binary search if the range grows.
    const i = bounds.findIndex((b) => b > run.started_at) - 1;
    for (const day of [row.days[i]!, fleet.days[i]!]) {
      if (run.status === "success") day.success += 1;
      else if (run.status === "failed") day.failed += 1;
      else if (run.status === "running") day.running = true;
    }
  }

  const rows = [...byId.values()].map(finish);
  rows.sort((a, b) => (a.uptime ?? Infinity) - (b.uptime ?? Infinity) || a.title.localeCompare(b.title));
  finish(fleet);

  const oldest = runs.length === 0 ? Infinity : Math.min(...runs.map((r) => r.started_at));
  return {
    rows,
    fleet,
    daysWithFailures: fleet.days.filter((d) => d.failed > 0).length,
    perfect: rows.filter((r) => r.uptime === 1).length,
    partialSample: runs.length >= fetchLimit && oldest > start,
  };
}

/** Uptime as a percentage with one decimal ("99.5%"), or "—" with no finished run. */
export function uptimeLabel(uptime: number | null): string {
  if (uptime === null) return "—";
  const pct = Math.floor(uptime * 1000) / 10;
  return `${pct === 100 ? "100" : pct.toFixed(1)}%`;
}

/** Tooltip / accessible label for one day bar. */
export function dayLabel(day: DayCell): string {
  const date = new Date(day.from * 1000).toLocaleDateString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
  if (day.state === "none") return `${date}: no finished runs${day.running ? " (running now)" : ""}`;
  return `${date}: ${day.success} succeeded, ${day.failed} failed${day.running ? ", 1+ running" : ""}`;
}

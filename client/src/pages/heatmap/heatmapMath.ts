/**
 * Pure aggregation logic behind the schedule heatmap: the 7×24 fire-density
 * grid, the color-ramp bucketing, the axis totals, and the derived
 * "busiest window" / day labels. Ported 1:1 from
 * `ui/src/schedule_heatmap_grid.rs` — see that file (and its
 * `schedule_heatmap_tests.rs`) for the reference behavior every function here
 * must match. Free of any DOM dependency so it is unit-testable directly.
 */
import type { RoutineResponse } from "../../api/hooks";
import { dateOnly, parseSchedule, WEEKDAYS } from "../../lib/schedule";

/** Rows in the grid: the next 7 calendar days, row 0 = today. */
export const HEAT_DAYS = 7;
/** Columns in the grid: the 24 hours of the day. */
export const HEAT_HOURS = 24;
/** Upper bound on fire-time iterations per source over the window. An
 * every-minute schedule fires 7×1440 = 10 080 times/week; this leaves headroom
 * while bounding cost on pathological (e.g. per-second) inputs. */
const MAX_FIRES_PER_SOURCE = 20_000;

/** Only source kind today; kept distinct from the `HeatFilter` union so future
 * kinds slot in without reshaping either type. */
export type HeatKind = "routine";

/** Source-kind filter for the grid: "all" counts every source, "routine" counts
 * only routines (currently equivalent to "all" since routines are the only kind). */
export type HeatFilter = "all" | "routine";

/** Short uppercase label for the filter toggle button. */
export function heatFilterLabel(filter: HeatFilter): string {
  return filter === "all" ? "ALL" : "ROUTINES";
}

/** Whether a source of `kind` is counted under `filter`. */
export function heatFilterAccepts(filter: HeatFilter, kind: HeatKind): boolean {
  return filter === "all" || filter === kind;
}

/** A schedule-bearing entity reduced to just what the heatmap needs. */
export interface HeatSource {
  kind: HeatKind;
  schedule: string;
  enabled: boolean;
}

/** The computed 7×24 density grid plus derived stats. */
export interface Heatmap {
  /** `grid[day][hour]` = number of fires; day 0 = today. */
  grid: number[][];
  /** Total fires across the whole window. */
  total: number;
  /** Largest single-cell count — the color-ramp denominator. */
  maxCell: number;
  /** `[day, hour]` of the busiest cell, or `undefined` when nothing fires. */
  peak: [number, number] | undefined;
  /** Number of enabled, filter-matching sources that contributed at least one fire. */
  sources: number;
}

/** Aggregate the next-7-day fire density of every enabled source matching
 * `filter`, bucketed by `(day, hour)` with day 0 = `now`'s calendar day. Fires
 * are counted strictly after `now`, so hours already elapsed today read empty. */
export function computeHeatmap(sources: HeatSource[], now: Date, filter: HeatFilter): Heatmap {
  const today = dateOnly(now);
  const endDate = new Date(today.getFullYear(), today.getMonth(), today.getDate() + HEAT_DAYS);
  const grid: number[][] = Array.from({ length: HEAT_DAYS }, () => new Array<number>(HEAT_HOURS).fill(0));
  let sourcesCounted = 0;

  for (const source of sources) {
    if (!source.enabled || !heatFilterAccepts(filter, source.kind)) continue;
    const cron = parseSchedule(source.schedule, now);
    if (!cron) continue;
    let contributed = false;
    // Fires strictly after `now` in chronological order, so each `date` is on or
    // after `today`; stop at the first fire that lands on or past the window's
    // end. The iteration cap bounds cost on pathological (sub-minute) schedules.
    for (let i = 0; i < MAX_FIRES_PER_SOURCE && cron.hasNext(); i++) {
      const dt = cron.next().toDate();
      const date = dateOnly(dt);
      if (date.getTime() >= endDate.getTime()) break;
      const day = Math.round((date.getTime() - today.getTime()) / 86_400_000);
      const row = grid[day];
      if (!row) continue;
      row[dt.getHours()] = (row[dt.getHours()] ?? 0) + 1;
      contributed = true;
    }
    if (contributed) sourcesCounted++;
  }

  let total = 0;
  let maxCell = 0;
  let peak: [number, number] | undefined;
  for (let day = 0; day < HEAT_DAYS; day++) {
    const row = grid[day];
    if (!row) continue;
    for (let hour = 0; hour < HEAT_HOURS; hour++) {
      const count = row[hour] ?? 0;
      total += count;
      if (count > maxCell) {
        maxCell = count;
        peak = [day, hour];
      }
    }
  }

  return { grid, total, maxCell, peak, sources: sourcesCounted };
}

/** The 0–4 color-ramp bucket for `count` relative to the grid's `max` cell.
 * 0 = empty; 1–4 split the non-empty range into quartiles so the busiest cells
 * reach the top of the ramp. */
export function intensityLevel(count: number, max: number): number {
  if (count === 0 || max === 0) return 0;
  const ratio = count / max;
  return Math.min(4, Math.max(1, Math.ceil(ratio * 4)));
}

/** Per-day fire totals (length [`HEAT_DAYS`]). */
export function dayTotals(map: Heatmap): number[] {
  return map.grid.map((hours) => hours.reduce((a, b) => a + b, 0));
}

/** Per-hour fire totals across all days (length [`HEAT_HOURS`]). */
export function hourTotals(map: Heatmap): number[] {
  return Array.from({ length: HEAT_HOURS }, (_, hour) =>
    map.grid.reduce((sum, day) => sum + (day[hour] ?? 0), 0),
  );
}

/** How many of the grid's cells hold at least one fire. */
export function filledCells(map: Heatmap): number {
  return map.grid.flat().filter((count) => count > 0).length;
}

/** Index into [`WEEKDAYS`] for `date`, i.e. `getDay()` (0 = Sunday). */
function weekdayIndex(date: Date): number {
  return date.getDay();
}

/** Weekday abbreviation for the row `day` days after `today`. */
function weekdayOf(today: Date, day: number): string {
  const date = new Date(today.getFullYear(), today.getMonth(), today.getDate() + day);
  return WEEKDAYS[weekdayIndex(date)] ?? "";
}

/** Human label for the busiest window, e.g. "THU 14:00 · 3 runs", or `undefined`
 * when the grid is empty. */
export function peakLabel(map: Heatmap, today: Date): string | undefined {
  if (!map.peak) return undefined;
  const [day, hour] = map.peak;
  const count = map.grid[day]?.[hour] ?? 0;
  const plural = count === 1 ? "run" : "runs";
  return `${weekdayOf(today, day)} ${String(hour).padStart(2, "0")}:00 · ${count} ${plural}`;
}

/** `"MON 23"`-style label for grid row `day`, counting forward from `today`. */
export function dayLabel(today: Date, day: number): string {
  const date = new Date(today.getFullYear(), today.getMonth(), today.getDate() + day);
  return `${WEEKDAYS[weekdayIndex(date)]} ${date.getDate()}`;
}

/** Map the routine record list into one `HeatSource` array. */
export function sourcesOf(routines: RoutineResponse[]): HeatSource[] {
  return routines.map((r) => ({ kind: "routine", schedule: r.schedule, enabled: r.enabled }));
}

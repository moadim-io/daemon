/**
 * Pure math for the USAGE page: how much agent wall-clock time the fleet burns per day, which
 * routine or agent consumes it, and whether that is growing versus the previous period.
 *
 * Best practice (Dagster+ Insights, Datadog Jobs Monitoring): track historical execution time per
 * job next to the operational data people already check, with a period-over-period trend, so a
 * runaway routine shows up before it shows up on the bill.
 */
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { dayBounds } from "../uptime/uptimeMath";

export type UsageGroupBy = "routine" | "agent";

/** `?by=` value, or `routine` when missing or unknown. */
export function groupByFromParam(value: string | null): UsageGroupBy {
  return value === "agent" ? "agent" : "routine";
}

export interface UsageDay {
  /** Local midnight, epoch seconds. */
  from: number;
  /** Next local midnight, epoch seconds (exclusive). */
  to: number;
  successSecs: number;
  failedSecs: number;
  /** Running, killed or unknown runs. */
  otherSecs: number;
}

export interface UsageRow {
  /** Routine id, or agent name. */
  key: string;
  label: string;
  secs: number;
  runs: number;
  /** Same key's agent time in the previous period of equal length. */
  prevSecs: number;
}

export interface UsageReport {
  days: UsageDay[];
  /** Biggest consumer first. */
  rows: UsageRow[];
  totalSecs: number;
  prevTotalSecs: number;
  runs: number;
  /** The capped run sample does not reach back to the start of the previous period. */
  partialSample: boolean;
}

/** Agent wall-clock seconds of one run: finish − start, or elapsed-so-far while running, else 0. */
export function runSecs(run: FleetRunSummary, nowSecs: number): number {
  if (run.finished_at != null) return Math.max(0, run.finished_at - run.started_at);
  return run.status === "running" ? Math.max(0, nowSecs - run.started_at) : 0;
}

export function buildUsage(
  runs: FleetRunSummary[],
  routines: RoutineResponse[],
  nowSecs: number,
  days: number,
  groupBy: UsageGroupBy,
  fetchLimit: number,
): UsageReport {
  // One bounds array over both periods: [prevStart … start … end].
  const bounds = dayBounds(nowSecs, days * 2);
  const prevStart = bounds[0]!;
  const start = bounds[days]!;
  const end = bounds[days * 2]!;
  const usageDays: UsageDay[] = bounds
    .slice(days, -1)
    .map((from, i) => ({ from, to: bounds[days + i + 1]!, successSecs: 0, failedSecs: 0, otherSecs: 0 }));

  const agentOf = new Map(routines.map((r) => [r.id, r.agent]));
  const titleOf = new Map(routines.map((r) => [r.id, r.title]));
  const rowsByKey = new Map<string, UsageRow>();
  let totalSecs = 0;
  let prevTotalSecs = 0;
  let count = 0;

  for (const run of runs) {
    if (run.started_at < prevStart || run.started_at >= end) continue;
    const secs = runSecs(run, nowSecs);
    const key = groupBy === "agent" ? (agentOf.get(run.routine_id) ?? "unknown") : run.routine_id;
    let row = rowsByKey.get(key);
    if (!row) {
      // A deleted routine still has runs on disk: keep it, under the title it ran with.
      const label = groupBy === "agent" ? key : (titleOf.get(key) ?? run.routine_title);
      row = { key, label, secs: 0, runs: 0, prevSecs: 0 };
      rowsByKey.set(key, row);
    }
    if (run.started_at < start) {
      row.prevSecs += secs;
      prevTotalSecs += secs;
      continue;
    }
    // ponytail: a run's whole duration lands on its start day; split across midnight if runs get long.
    // ponytail: linear scan over ≤ 90 days per run; binary search if the range grows.
    const day = usageDays.find((d) => run.started_at < d.to)!;
    if (run.status === "success") day.successSecs += secs;
    else if (run.status === "failed") day.failedSecs += secs;
    else day.otherSecs += secs;
    row.secs += secs;
    row.runs += 1;
    totalSecs += secs;
    count += 1;
  }

  const rows = [...rowsByKey.values()];
  rows.sort((a, b) => b.secs - a.secs || b.prevSecs - a.prevSecs || a.label.localeCompare(b.label));
  const oldest = runs.length === 0 ? -Infinity : Math.min(...runs.map((r) => r.started_at));
  return {
    days: usageDays,
    rows,
    totalSecs,
    prevTotalSecs,
    runs: count,
    partialSample: runs.length >= fetchLimit && oldest > prevStart,
  };
}

/** Agent time as `"0m"`, `"42m"`, `"3h 5m"` or `"212h"` (minutes dropped from 100 h). */
export function fmtAgentTime(secs: number): string {
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m`;
  const h = Math.floor(mins / 60);
  return h >= 100 ? `${h}h` : `${h}h ${mins % 60}m`;
}

/** Signed percent change, `"new"` from zero, `"—"` when both are zero. */
export function changeLabel(cur: number, prev: number): string {
  if (prev === 0) return cur === 0 ? "—" : "new";
  const pct = Math.round(((cur - prev) / prev) * 100);
  return `${pct > 0 ? "+" : ""}${pct}%`;
}

/** Red when agent time grew more than 10% (cost up); smaller moves are noise. */
export function changeClass(cur: number, prev: number): string | undefined {
  return prev > 0 && (cur - prev) / prev > 0.1 ? "c-red" : undefined;
}

/** Tooltip / accessible label for one day bar. */
export function usageDayLabel(day: UsageDay): string {
  const date = new Date(day.from * 1000).toLocaleDateString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
  const total = day.successSecs + day.failedSecs + day.otherSecs;
  if (total === 0) return `${date}: no agent time`;
  return `${date}: ${fmtAgentTime(total)} (${fmtAgentTime(day.successSecs)} succeeded, ${fmtAgentTime(day.failedSecs)} failed, ${fmtAgentTime(day.otherSecs)} other)`;
}

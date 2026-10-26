/**
 * SLO error budgets for the RELIABILITY page: given a success-rate target, how much of each
 * routine's allowed failures over a rolling 7-day window is already spent, and how fast.
 *
 * Best practice (Google SRE workbook, SLO burn-rate dashboards): show the SLI, the target, the
 * remaining budget and the burn rate together. Burn rate = error rate / (1 - target); 1 spends
 * the budget exactly at the window end. A short window against the long window (multi-window
 * burn rate) catches a fast regression before the 7-day budget is gone.
 */
import type { FleetRunSummary } from "../../api/hooks";

export const SLO_WINDOW_SECS = 7 * 24 * 60 * 60;
export const SHORT_WINDOW_SECS = 24 * 60 * 60;

export const SLO_TARGETS = [0.9, 0.95, 0.99, 0.999] as const;
export const DEFAULT_SLO_TARGET = 0.95;

/** 24h burn at or above this spends the 7-day budget in 3.5 days or less. */
export const FAST_BURN = 2;

/** Budget left below this share counts as at risk. */
export const AT_RISK_LEFT = 0.25;

export type BudgetStatus = "exhausted" | "fast-burn" | "at-risk" | "ok";

export interface RoutineBudget {
  routineId: string;
  routineTitle: string;
  runs: number;
  failures: number;
  /** Failures the target allows over the window's runs (fractional). */
  allowed: number;
  /** 1 - failures / allowed; negative once overspent. */
  budgetLeft: number;
  burn7d: number;
  /** `null` with no finished run in the last 24h. */
  burn24h: number | null;
  status: BudgetStatus;
}

export interface SloReport {
  /** Routines with a finished run in the window, worst status first. */
  budgets: RoutineBudget[];
  exhaustedCount: number;
  /** The capped sample does not reach back to the window's start. */
  partial: boolean;
}

const STATUS_ORDER: BudgetStatus[] = ["exhausted", "fast-burn", "at-risk", "ok"];

const SLO_KEY = "moadim.client.reliability.slo";

/** The persisted target, or the default when unset, unknown or storage is unavailable. */
export function loadSloTarget(): number {
  try {
    const v = Number(localStorage.getItem(SLO_KEY));
    return (SLO_TARGETS as readonly number[]).includes(v) ? v : DEFAULT_SLO_TARGET;
  } catch {
    return DEFAULT_SLO_TARGET;
  }
}

export function saveSloTarget(target: number): void {
  try {
    localStorage.setItem(SLO_KEY, String(target));
  } catch {
    // storage unavailable — the in-memory choice still applies
  }
}

/** "95%", "99.9%". */
export function targetLabel(target: number): string {
  return `${Math.round(target * 1000) / 10}%`;
}

function statusOf(budgetLeft: number, failures: number, burn24h: number | null): BudgetStatus {
  if (failures > 0 && budgetLeft <= 0) return "exhausted";
  if (burn24h !== null && burn24h >= FAST_BURN) return "fast-burn";
  if (budgetLeft < AT_RISK_LEFT) return "at-risk";
  return "ok";
}

/**
 * Counts finished (`success`/`failed`) runs started in `[now - 7d, now]` per routine and derives
 * each budget against `target`. `fetchLimit` is the cap the sample was fetched with: a full sample
 * whose oldest run is newer than the window start is truncated.
 */
export function computeSlo(runs: FleetRunSummary[], now: number, target: number, fetchLimit: number): SloReport {
  const start = now - SLO_WINDOW_SECS;
  const shortStart = now - SHORT_WINDOW_SECS;
  const allowedRate = 1 - target;
  const byRoutine = new Map<string, { title: string; runs: number; failures: number; runs24: number; failures24: number }>();
  let oldest = Infinity;
  for (const run of runs) {
    oldest = Math.min(oldest, run.started_at);
    if (run.status !== "success" && run.status !== "failed") continue;
    if (run.started_at < start || run.started_at > now) continue;
    let e = byRoutine.get(run.routine_id);
    if (!e) {
      e = { title: run.routine_title, runs: 0, failures: 0, runs24: 0, failures24: 0 };
      byRoutine.set(run.routine_id, e);
    }
    const failed = run.status === "failed" ? 1 : 0;
    e.runs += 1;
    e.failures += failed;
    if (run.started_at >= shortStart) {
      e.runs24 += 1;
      e.failures24 += failed;
    }
  }
  const budgets: RoutineBudget[] = [...byRoutine].map(([routineId, e]) => {
    const allowed = e.runs * allowedRate;
    const budgetLeft = 1 - e.failures / allowed;
    const burn24h = e.runs24 === 0 ? null : e.failures24 / e.runs24 / allowedRate;
    return {
      routineId,
      routineTitle: e.title,
      runs: e.runs,
      failures: e.failures,
      allowed,
      budgetLeft,
      burn7d: e.failures / e.runs / allowedRate,
      burn24h,
      status: statusOf(budgetLeft, e.failures, burn24h),
    };
  });
  budgets.sort(
    (a, b) =>
      STATUS_ORDER.indexOf(a.status) - STATUS_ORDER.indexOf(b.status) ||
      a.budgetLeft - b.budgetLeft ||
      a.routineTitle.localeCompare(b.routineTitle),
  );
  return {
    budgets,
    exhaustedCount: budgets.filter((b) => b.status === "exhausted").length,
    partial: runs.length >= fetchLimit && oldest > start,
  };
}

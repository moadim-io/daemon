/**
 * Week-over-week change for the RELIABILITY page: compares each routine's last 7 days against the
 * 7 days before, so an operator sees what changed since last week instead of a run-count sample.
 *
 * Best practice (GitLab CI/CD analytics, SLO burn-rate dashboards): judge reliability over fixed
 * time windows as ratios, not raw counts, and compare a short window against a baseline window.
 * Cron monitors (Cronitor, Healthchecks) also treat a job that stops reporting as a signal.
 */
import type { FleetRunSummary } from "../../api/hooks";

export const WEEK_SECS = 7 * 24 * 60 * 60;

/** Success rate must drop by at least this many percentage points to count as worse. */
export const RATE_DELTA_PP = 10;

/** p50 duration must change by at least this factor to count as slower/faster. */
export const P50_RATIO = 1.25;

/** Timed runs needed in each window before a p50 change is judged. */
export const MIN_TIMED_RUNS = 3;

export type Verdict = "worse" | "quiet" | "new" | "better" | "steady";

export interface WindowStats {
  runs: number;
  failures: number;
  /** `null` with no finished run in the window. */
  rate: number | null;
  /** `null` with fewer than `MIN_TIMED_RUNS` timed runs. */
  p50Secs: number | null;
}

export interface RoutineChange {
  routineId: string;
  routineTitle: string;
  current: WindowStats;
  prior: WindowStats;
  /** Current minus prior success rate, in percentage points; `null` when either is unknown. */
  rateDeltaPp: number | null;
  /** Current p50 over prior p50; `null` when either is unknown. */
  p50Ratio: number | null;
  verdict: Verdict;
}

export interface WeekOverWeekReport {
  /** Routines with a finished run in either window, worst verdict first. */
  changes: RoutineChange[];
  fleetCurrent: WindowStats;
  fleetPrior: WindowStats;
  /** The capped sample does not reach back to the start of the prior window. */
  priorPartial: boolean;
}

const VERDICT_ORDER: Verdict[] = ["worse", "quiet", "new", "better", "steady"];

interface Acc {
  runs: number;
  failures: number;
  durations: number[];
}

const emptyAcc = (): Acc => ({ runs: 0, failures: 0, durations: [] });

function median(values: number[]): number | null {
  if (values.length < MIN_TIMED_RUNS) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.ceil(sorted.length / 2) - 1] ?? null;
}

function stats(acc: Acc): WindowStats {
  return {
    runs: acc.runs,
    failures: acc.failures,
    rate: acc.runs === 0 ? null : (acc.runs - acc.failures) / acc.runs,
    p50Secs: median(acc.durations),
  };
}

function verdictOf(current: WindowStats, prior: WindowStats, rateDeltaPp: number | null, p50Ratio: number | null) {
  if (current.runs === 0) return "quiet";
  if (prior.runs === 0) return "new";
  if ((rateDeltaPp !== null && rateDeltaPp <= -RATE_DELTA_PP) || (p50Ratio !== null && p50Ratio >= P50_RATIO)) {
    return "worse";
  }
  if ((rateDeltaPp !== null && rateDeltaPp >= RATE_DELTA_PP) || (p50Ratio !== null && p50Ratio <= 1 / P50_RATIO)) {
    return "better";
  }
  return "steady";
}

/**
 * Buckets finished (`success`/`failed`) runs into the current `[now - 7d, now]` and prior
 * `[now - 14d, now - 7d)` windows by start time. `fetchLimit` is the cap the sample was fetched
 * with: a full sample whose oldest run is newer than the prior window's start is truncated.
 */
export function computeWeekOverWeek(runs: FleetRunSummary[], now: number, fetchLimit: number): WeekOverWeekReport {
  const currentStart = now - WEEK_SECS;
  const priorStart = now - 2 * WEEK_SECS;
  const byRoutine = new Map<string, { title: string; current: Acc; prior: Acc }>();
  const fleet = { current: emptyAcc(), prior: emptyAcc() };
  let oldest = Infinity;
  for (const run of runs) {
    oldest = Math.min(oldest, run.started_at);
    if (run.status !== "success" && run.status !== "failed") continue;
    if (run.started_at < priorStart || run.started_at > now) continue;
    const key = run.started_at >= currentStart ? "current" : "prior";
    let entry = byRoutine.get(run.routine_id);
    if (!entry) {
      entry = { title: run.routine_title, current: emptyAcc(), prior: emptyAcc() };
      byRoutine.set(run.routine_id, entry);
    }
    for (const acc of [entry[key], fleet[key]]) {
      acc.runs += 1;
      if (run.status === "failed") acc.failures += 1;
      if (run.finished_at != null && run.finished_at >= run.started_at) acc.durations.push(run.finished_at - run.started_at);
    }
  }
  const changes: RoutineChange[] = [...byRoutine].map(([routineId, e]) => {
    const current = stats(e.current);
    const prior = stats(e.prior);
    const rateDeltaPp = current.rate === null || prior.rate === null ? null : (current.rate - prior.rate) * 100;
    const p50Ratio =
      current.p50Secs === null || prior.p50Secs === null || prior.p50Secs <= 0 ? null : current.p50Secs / prior.p50Secs;
    return {
      routineId,
      routineTitle: e.title,
      current,
      prior,
      rateDeltaPp,
      p50Ratio,
      verdict: verdictOf(current, prior, rateDeltaPp, p50Ratio),
    };
  });
  changes.sort(
    (a, b) =>
      VERDICT_ORDER.indexOf(a.verdict) - VERDICT_ORDER.indexOf(b.verdict) ||
      (a.rateDeltaPp ?? 0) - (b.rateDeltaPp ?? 0) ||
      a.routineTitle.localeCompare(b.routineTitle),
  );
  return {
    changes,
    fleetCurrent: stats(fleet.current),
    fleetPrior: stats(fleet.prior),
    priorPartial: runs.length >= fetchLimit && oldest > priorStart,
  };
}

/**
 * Pure reliability-metrics computation for the RELIABILITY page: per-routine success rate,
 * active pass/fail streak, flakiness (status-flip rate), and run-duration percentiles with a
 * slower-trend regression flag, over each routine's most recent finished runs.
 *
 * Best practice (CI/CD reliability dashboards — GitHub Actions Insights, CircleCI Insights,
 * Datadog CI Visibility): rank jobs by recent success rate, flag flaky ones (alternating
 * pass/fail) separately from steadily-failing ones, and track p50/p95 duration plus a
 * recent-vs-baseline comparison — p50 shows the typical run, p95 exposes worst-case outliers an
 * average hides, and the trend comparison catches a regression before it escalates into
 * failures. Ports `ui/src/reliability_stats.rs`'s ranking/flakiness math (pre-dating the removal
 * of the Yew UI in favor of this React client) and extends it with the duration dimension.
 */
import type { FleetRunSummary } from "../../api/hooks";

/** Most recent finished runs per routine considered for reliability metrics. */
export const SAMPLE_LEN = 20;

/** Minimum finished-run sample before flakiness is judged. */
const FLAKY_MIN_SAMPLE = 5;

/** Adjacent-pair flip ratio at/above which a routine is flagged flaky. */
const FLAKY_FLIP_RATIO = 0.4;

/** Minimum timed-run sample before a duration regression is judged (needs two stable halves). */
const REGRESSION_MIN_SAMPLE = 8;

/** Newer-half median duration must exceed the older half's by this factor to flag a regression. */
const REGRESSION_RATIO = 1.25;

export type Streak =
  | { kind: "success"; count: number }
  | { kind: "failure"; count: number }
  | { kind: "none" };

export interface RoutineReliability {
  routineId: string;
  routineTitle: string;
  /** Finished runs considered, out of up to `SAMPLE_LEN`. */
  sampleSize: number;
  /** `success` runs within the sample. */
  successes: number;
  streak: Streak;
  /** Count of status flips across adjacent runs in the sample. */
  flips: number;
  /** Finished runs' durations in seconds, newest-first (only runs with both timestamps). */
  durationsSecs: number[];
  p50Secs: number | null;
  p95Secs: number | null;
  /** `true` when this routine's recent runs are meaningfully slower than its baseline. */
  regressing: boolean;
}

export interface FleetReliability {
  sampleSize: number;
  successes: number;
  /** Routines with an active (>= 1 run) failure streak. */
  failingCount: number;
  flakyCount: number;
  p50Secs: number | null;
  p95Secs: number | null;
  regressingCount: number;
}

/** `successes / sampleSize`, or `null` when the sample is empty. */
export function successRate(item: RoutineReliability): number | null {
  return item.sampleSize === 0 ? null : item.successes / item.sampleSize;
}

/** `true` when a routine's flip rate crosses the flaky threshold. */
export function isFlaky(item: RoutineReliability): boolean {
  if (item.sampleSize < FLAKY_MIN_SAMPLE) return false;
  const pairs = item.sampleSize - 1;
  return pairs > 0 && item.flips / pairs >= FLAKY_FLIP_RATIO;
}

/** The length of an active failure streak, or 0 for a success streak or no sample. */
function failureStreakLen(streak: Streak): number {
  return streak.kind === "failure" ? streak.count : 0;
}

/** The active streak at the head (newest-first) of a finished-run status list. */
function computeStreak(statuses: FleetRunSummary["status"][]): Streak {
  const newest = statuses[0];
  if (newest === undefined) return { kind: "none" };
  let n = 0;
  for (const s of statuses) {
    if (s !== newest) break;
    n += 1;
  }
  return newest === "success" ? { kind: "success", count: n } : { kind: "failure", count: n };
}

/** Count of adjacent-pair status changes in a finished-run status list. */
function countFlips(statuses: FleetRunSummary["status"][]): number {
  let flips = 0;
  for (let i = 1; i < statuses.length; i++) {
    if (statuses[i] !== statuses[i - 1]) flips += 1;
  }
  return flips;
}

/** Sample percentile via nearest-rank on an ascending-sorted array (empty input yields `null`). */
function percentile(sortedAsc: number[], p: number): number | null {
  if (sortedAsc.length === 0) return null;
  const idx = Math.min(sortedAsc.length - 1, Math.max(0, Math.ceil(p * sortedAsc.length) - 1));
  return sortedAsc[idx] ?? null;
}

/** Median of a value list, or `null` when empty. */
function median(values: number[]): number | null {
  return percentile([...values].sort((a, b) => a - b), 0.5);
}

/**
 * `true` when a newest-first duration sample's newer half runs meaningfully slower than its
 * older half — the same recent-vs-baseline comparison CI duration-drift tools use to catch a
 * performance regression before it escalates into failures.
 */
function isRegressing(durationsSecs: number[]): boolean {
  if (durationsSecs.length < REGRESSION_MIN_SAMPLE) return false;
  const half = Math.floor(durationsSecs.length / 2);
  const newer = median(durationsSecs.slice(0, half));
  const older = median(durationsSecs.slice(half));
  if (newer === null || older === null || older <= 0) return false;
  return newer > older * REGRESSION_RATIO;
}

interface RoutineAcc {
  title: string;
  /** Newest-first, capped at `SAMPLE_LEN`. */
  statuses: FleetRunSummary["status"][];
  /** Newest-first, capped at `SAMPLE_LEN`; only runs with both timestamps. */
  durationsSecs: number[];
}

/**
 * Buckets a fleet-wide, newest-first run list by routine, keeping only `success`/`failed` runs
 * and capping each bucket's statuses/durations at `SAMPLE_LEN`, newest-first.
 */
function bucketFinishedRuns(runs: FleetRunSummary[]): Map<string, RoutineAcc> {
  const byRoutine = new Map<string, RoutineAcc>();
  for (const run of runs) {
    if (run.status !== "success" && run.status !== "failed") continue;
    let acc = byRoutine.get(run.routine_id);
    if (!acc) {
      acc = { title: run.routine_title, statuses: [], durationsSecs: [] };
      byRoutine.set(run.routine_id, acc);
    }
    if (acc.statuses.length < SAMPLE_LEN) acc.statuses.push(run.status);
    if (acc.durationsSecs.length < SAMPLE_LEN && run.finished_at != null && run.finished_at >= run.started_at) {
      acc.durationsSecs.push(run.finished_at - run.started_at);
    }
  }
  return byRoutine;
}

/**
 * Computes every routine's reliability metrics from a fleet-wide run list, ranked worst-first:
 * an active failure streak outranks everything else (longer streak first), then lowest success
 * rate, then title for a stable tie-break. Routines with no finished run in the sample are
 * omitted — there is nothing to rank.
 */
export function computeReliability(runs: FleetRunSummary[]): RoutineReliability[] {
  const items: RoutineReliability[] = [];
  for (const [routineId, acc] of bucketFinishedRuns(runs)) {
    const sortedDurations = [...acc.durationsSecs].sort((a, b) => a - b);
    items.push({
      routineId,
      routineTitle: acc.title,
      sampleSize: acc.statuses.length,
      successes: acc.statuses.filter((s) => s === "success").length,
      streak: computeStreak(acc.statuses),
      flips: countFlips(acc.statuses),
      durationsSecs: acc.durationsSecs,
      p50Secs: percentile(sortedDurations, 0.5),
      p95Secs: percentile(sortedDurations, 0.95),
      regressing: isRegressing(acc.durationsSecs),
    });
  }

  items.sort((a, b) => {
    const streakDiff = failureStreakLen(b.streak) - failureStreakLen(a.streak);
    if (streakDiff !== 0) return streakDiff;
    const aRate = successRate(a) ?? 1;
    const bRate = successRate(b) ?? 1;
    if (aRate !== bRate) return aRate - bRate;
    return a.routineTitle.localeCompare(b.routineTitle);
  });
  return items;
}

/** Aggregates per-routine metrics into a fleet-wide summary for the page's stat tiles. */
export function fleetSummary(items: RoutineReliability[]): FleetReliability {
  const allDurations = items
    .flatMap((r) => r.durationsSecs)
    .sort((a, b) => a - b);
  return {
    sampleSize: items.reduce((sum, r) => sum + r.sampleSize, 0),
    successes: items.reduce((sum, r) => sum + r.successes, 0),
    failingCount: items.filter((r) => failureStreakLen(r.streak) > 0).length,
    flakyCount: items.filter(isFlaky).length,
    p50Secs: percentile(allDurations, 0.5),
    p95Secs: percentile(allDurations, 0.95),
    regressingCount: items.filter((r) => r.regressing).length,
  };
}

/** CSS class for a routine's streak badge (reuses the generic `run-status` pill classes). */
export function streakClass(streak: Streak): string {
  switch (streak.kind) {
    case "success":
      return "run-status success";
    case "failure":
      return "run-status failed";
    default:
      return "run-status unknown";
  }
}

/** Display label for a routine's streak badge. */
export function streakLabel(streak: Streak): string {
  switch (streak.kind) {
    case "success":
      return `${streak.count} OK`;
    case "failure":
      return `${streak.count} FAILING`;
    default:
      return "—";
  }
}

/** CSS class for a success-rate badge (reuses the generic `run-status` pill classes). */
export function rateClass(rate: number | null): string {
  if (rate === null) return "run-status unknown";
  if (rate >= 0.9) return "run-status success";
  if (rate >= 0.7) return "run-status running";
  return "run-status failed";
}

/** Display label for a success-rate badge. */
export function rateLabel(rate: number | null): string {
  return rate === null ? "—" : `${Math.round(rate * 100)}%`;
}

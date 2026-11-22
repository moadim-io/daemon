import type { FleetRunSummary } from "../../api/hooks";

/** localStorage key for the unix-seconds baseline the SINCE YOUR LAST VISIT digest counts from. */
const LAST_SEEN_KEY = "moadim.client.overview.lastSeen";

/** One routine that failed at least once since the baseline. */
export interface DigestRoutine {
  routineId: string;
  routineTitle: string;
  /** Failed runs that finished since the baseline. */
  failures: number;
  /** The newest of those failed runs — the row links to its detail page. */
  latestFailure: FleetRunSummary;
  /** The routine's newest finished run (any time) succeeded, so it has since recovered. */
  recovered: boolean;
}

export interface VisitDigest {
  failed: number;
  succeeded: number;
  /** Still-failing routines first, then recovered; each group newest failure first. */
  routines: DigestRoutine[];
  /** The fetch hit its cap before reaching the baseline, so counts may be undercounted. */
  truncated: boolean;
}

/**
 * Summarizes the runs that finished after `sinceSecs`. `runs` is `GET /routines/runs`'s
 * newest-first list, so the first finished run seen per routine is its latest outcome.
 */
export function computeDigest(runs: FleetRunSummary[], sinceSecs: number, fetchLimit: number): VisitDigest {
  let failed = 0;
  let succeeded = 0;
  const latestOutcome = new Map<string, FleetRunSummary["status"]>();
  const byRoutine = new Map<string, DigestRoutine>();
  for (const run of runs) {
    if (run.finished_at == null) continue;
    if (!latestOutcome.has(run.routine_id)) latestOutcome.set(run.routine_id, run.status);
    if (run.finished_at <= sinceSecs) continue;
    if (run.status === "success") succeeded++;
    if (run.status !== "failed") continue;
    failed++;
    const entry = byRoutine.get(run.routine_id);
    if (entry) {
      entry.failures++;
    } else {
      byRoutine.set(run.routine_id, {
        routineId: run.routine_id,
        routineTitle: run.routine_title,
        failures: 1,
        latestFailure: run,
        recovered: latestOutcome.get(run.routine_id) === "success",
      });
    }
  }
  const routines = [...byRoutine.values()].sort(
    (a, b) =>
      Number(a.recovered) - Number(b.recovered) || (b.latestFailure.finished_at ?? 0) - (a.latestFailure.finished_at ?? 0),
  );
  const oldest = runs[runs.length - 1];
  const truncated = runs.length >= fetchLimit && oldest !== undefined && oldest.started_at > sinceSecs;
  return { failed, succeeded, routines, truncated };
}

/**
 * Reads the persisted baseline. A first visit (or unreadable storage) seeds it with `nowSecs`,
 * so the digest starts empty instead of replaying the whole fetched history.
 */
export function loadOrSeedLastSeen(nowSecs: number): number {
  try {
    const stored = Number(localStorage.getItem(LAST_SEEN_KEY));
    if (Number.isFinite(stored) && stored > 0) return stored;
  } catch {
    return nowSecs;
  }
  saveLastSeen(nowSecs);
  return nowSecs;
}

/** Persists the baseline (best-effort; ignores storage errors). */
export function saveLastSeen(secs: number): void {
  try {
    localStorage.setItem(LAST_SEEN_KEY, String(secs));
  } catch {
    // storage unavailable (private mode / quota) — in-memory baseline still applies
  }
}

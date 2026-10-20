/**
 * Failure incidents for the RELIABILITY page: each routine's consecutive `failed` runs grouped
 * into one episode, opened at the first failure's start and resolved when the next `success` run
 * finishes — plus the fleet's mean time to recover (MTTR) over resolved episodes.
 *
 * Best practice (DORA "time to restore service", SRE incident timelines, cron monitors like
 * Cronitor): treat a failure as an episode with an open/resolved state and measure the clock from
 * first failure to verified recovery, so one quick blip and a two-day outage stop looking alike
 * in success-rate terms.
 */
import type { FleetRunSummary } from "../../api/hooks";

export interface Incident {
  routineId: string;
  routineTitle: string;
  /** Start of the first failed run in the episode (unix seconds). */
  openedAt: number;
  /** Finish of the recovering `success` run, or `null` while still failing. */
  resolvedAt: number | null;
  /** `resolvedAt - openedAt`, or `now - openedAt` while ongoing. */
  durationSecs: number;
  failedRuns: number;
  /** Workbench of the first failed run, for the Run Detail link. */
  firstWorkbench: string;
  /** The first failure is the routine's oldest run in the fetched sample — it may have started earlier. */
  truncated: boolean;
}

export interface IncidentReport {
  /** Ongoing first, then newest-opened first. */
  incidents: Incident[];
  openCount: number;
  /** Mean recovery time over resolved incidents, `null` when none resolved. */
  mttrSecs: number | null;
  /** Longest resolved recovery, `null` when none resolved. */
  longestSecs: number | null;
}

/** Groups a fleet-wide run list (any order) into per-routine failure incidents. */
export function computeIncidents(runs: FleetRunSummary[], now: number): IncidentReport {
  const byRoutine = new Map<string, FleetRunSummary[]>();
  for (const run of runs) {
    if (run.status !== "success" && run.status !== "failed") continue;
    const list = byRoutine.get(run.routine_id) ?? [];
    list.push(run);
    byRoutine.set(run.routine_id, list);
  }

  const incidents: Incident[] = [];
  for (const list of byRoutine.values()) {
    list.sort((a, b) => a.started_at - b.started_at);
    let open: Incident | null = null;
    for (const [i, run] of list.entries()) {
      if (run.status === "failed") {
        if (open) open.failedRuns += 1;
        else
          open = {
            routineId: run.routine_id,
            routineTitle: run.routine_title,
            openedAt: run.started_at,
            resolvedAt: null,
            durationSecs: 0,
            failedRuns: 1,
            firstWorkbench: run.workbench,
            truncated: i === 0,
          };
      } else if (open) {
        const resolvedAt = run.finished_at ?? run.started_at;
        incidents.push({ ...open, resolvedAt, durationSecs: Math.max(0, resolvedAt - open.openedAt) });
        open = null;
      }
    }
    if (open) incidents.push({ ...open, durationSecs: Math.max(0, now - open.openedAt) });
  }

  incidents.sort((a, b) => Number(a.resolvedAt !== null) - Number(b.resolvedAt !== null) || b.openedAt - a.openedAt);
  const resolved = incidents.filter((i) => i.resolvedAt !== null).map((i) => i.durationSecs);
  return {
    incidents,
    openCount: incidents.length - resolved.length,
    mttrSecs: resolved.length === 0 ? null : Math.round(resolved.reduce((s, d) => s + d, 0) / resolved.length),
    longestSecs: resolved.length === 0 ? null : Math.max(...resolved),
  };
}

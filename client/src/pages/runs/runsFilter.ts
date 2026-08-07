import type { FleetRunSummary } from "../../api/hooks";

export type RunStatusFacet = "all" | "success" | "failed" | "running" | "unknown";
export type RunTimeFacet = "all" | "1h" | "24h" | "7d";

export interface RunsFilter {
  query: string;
  status: RunStatusFacet;
  time: RunTimeFacet;
}

export const DEFAULT_RUNS_FILTER: RunsFilter = { query: "", status: "all", time: "all" };

const TIME_FACET_SECS: Record<Exclude<RunTimeFacet, "all">, number> = {
  "1h": 3_600,
  "24h": 86_400,
  "7d": 604_800,
};

/** Runs matching the free-text routine-title search, status facet, and recency window, newest-first. */
export function filterRuns(runs: FleetRunSummary[], filter: RunsFilter, nowSecs: number): FleetRunSummary[] {
  const q = filter.query.trim().toLowerCase();
  const cutoff = filter.time === "all" ? undefined : nowSecs - TIME_FACET_SECS[filter.time];
  return runs.filter((run) => {
    if (filter.status !== "all" && run.status !== filter.status) return false;
    if (cutoff !== undefined && run.started_at < cutoff) return false;
    if (q !== "" && !run.routine_title.toLowerCase().includes(q)) return false;
    return true;
  });
}

/** Count of runs per status, for the page's at-a-glance summary chips. */
export function statusCounts(runs: FleetRunSummary[]): Record<FleetRunSummary["status"], number> {
  const counts: Record<FleetRunSummary["status"], number> = { running: 0, success: 0, failed: 0, unknown: 0 };
  for (const run of runs) counts[run.status] += 1;
  return counts;
}

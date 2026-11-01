import type { FleetRunSummary } from "../../api/hooks";

export type RunStatusFacet = "all" | "success" | "failed" | "running" | "unknown";
export type RunTimeFacet = "all" | "1h" | "24h" | "7d";

export interface RunsFilter {
  query: string;
  status: RunStatusFacet;
  time: RunTimeFacet;
}

export const DEFAULT_RUNS_FILTER: RunsFilter = { query: "", status: "all", time: "all" };

export const TIME_FACET_SECS: Record<Exclude<RunTimeFacet, "all">, number> = {
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

const STATUS_FACETS: readonly string[] = ["all", "success", "failed", "running", "unknown"];
const TIME_FACETS: readonly string[] = ["all", "1h", "24h", "7d"];

/** Histogram bucket the user clicked, as epoch seconds. */
export interface RunsRange {
  from: number;
  to: number;
}

/** Restores the Runs view from `?q=&status=&when=&from=&to=`; unknown or missing values fall back to defaults. */
export function viewFromParams(params: URLSearchParams): { filter: RunsFilter; range: RunsRange | null } {
  const status = params.get("status") ?? "";
  const time = params.get("when") ?? "";
  const from = Number(params.get("from"));
  const to = Number(params.get("to"));
  const hasRange = params.has("from") && params.has("to") && Number.isFinite(from) && Number.isFinite(to) && from < to;
  return {
    filter: {
      query: params.get("q") ?? "",
      status: STATUS_FACETS.includes(status) ? (status as RunStatusFacet) : "all",
      time: TIME_FACETS.includes(time) ? (time as RunTimeFacet) : "all",
    },
    range: hasRange ? { from, to } : null,
  };
}

/** Inverse of {@link viewFromParams}; omits defaults so the plain `/runs` URL stays clean. */
export function viewToParams(filter: RunsFilter, range: RunsRange | null): URLSearchParams {
  const params = new URLSearchParams();
  if (filter.query !== "") params.set("q", filter.query);
  if (filter.status !== "all") params.set("status", filter.status);
  if (filter.time !== "all") params.set("when", filter.time);
  if (range !== null) {
    params.set("from", String(range.from));
    params.set("to", String(range.to));
  }
  return params;
}

function csvCell(value: string | number | null | undefined): string {
  const s = value == null ? "" : String(value);
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

function isoOrEmpty(secs: number | null | undefined): string {
  return secs == null ? "" : new Date(secs * 1000).toISOString();
}

const CSV_HEADER = "routine,routine_id,workbench,status,exit_code,started_at,finished_at,duration_secs";

/** RFC 4180 CSV of the given runs, in the given order, for the Runs page's Export CSV button. */
export function runsToCsv(runs: FleetRunSummary[]): string {
  const rows = runs.map((r) =>
    [
      r.routine_title,
      r.routine_id,
      r.workbench,
      r.status,
      r.exit_code,
      isoOrEmpty(r.started_at),
      isoOrEmpty(r.finished_at),
      r.finished_at == null ? null : r.finished_at - r.started_at,
    ]
      .map(csvCell)
      .join(","),
  );
  return [CSV_HEADER, ...rows].join("\r\n") + "\r\n";
}

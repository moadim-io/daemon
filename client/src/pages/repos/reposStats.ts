/**
 * Pure aggregation logic for the REPOSITORIES page: pivots every routine's `repositories` list
 * and the fleet-wide run feed into one row per repository — the "component" view software
 * catalogs ship (Backstage Software Catalog, Datadog Service Catalog) and the per-repository
 * health report CI monitoring guidance recommends, so one broken repository shows up as one row
 * instead of N failing routines. No new API calls or backend fields.
 */
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { percentile } from "../reliability/reliabilityStats";

export interface RepoRoutineRef {
  id: string;
  title: string;
}

export interface RepoStats {
  url: string;
  name: string;
  routines: RepoRoutineRef[];
  total: number;
  enabled: number;
  runningNow: number;
  /** Distinct, sorted branches; an entry with no branch counts as `default`. */
  branches: string[];
  /** Distinct, sorted agents of the routines that target this repository. */
  agents: string[];
  /** Routines whose entry for this repository sets `auto_pull: false`. */
  autoPullOff: number;
  /** Routines whose newest finished run failed. */
  failing: number;
  successCount: number;
  finishedCount: number;
  p50Secs: number | null;
  lastRun: { label: string; status: FleetRunSummary["status"] } | null;
}

/**
 * Short display name for a git remote: the last two path segments (`acme/api`) for URLs with at
 * least three parts (https and scp-style ssh alike), otherwise the input unchanged.
 */
export function repoShortName(url: string): string {
  const parts = url
    .trim()
    .replace(/\/+$/, "")
    .replace(/\.git$/, "")
    .split(/[/:]+/)
    .filter((p) => p !== "");
  return parts.length >= 3 ? parts.slice(-2).join("/") : url;
}

/**
 * One entry per distinct `repository` URL across all routines, ranked failing-first, then by
 * routine count (largest first), then name. `runs` is newest-first.
 */
export function computeRepoStats(routines: RoutineResponse[], runs: FleetRunSummary[]): RepoStats[] {
  const urls = [...new Set(routines.flatMap((r) => (r.repositories ?? []).map((e) => e.repository)))];
  const stats = urls.map((url): RepoStats => {
    const assigned = routines.filter((r) => (r.repositories ?? []).some((e) => e.repository === url));
    const entries = routines.flatMap((r) => (r.repositories ?? []).filter((e) => e.repository === url));
    const ids = new Set(assigned.map((r) => r.id));
    const repoRuns = runs.filter((run) => ids.has(run.routine_id));
    const finished = repoRuns.filter((run) => run.status === "success" || run.status === "failed");
    const durations = finished
      .flatMap((run) =>
        run.finished_at != null && run.finished_at >= run.started_at ? [run.finished_at - run.started_at] : [],
      )
      .sort((a, b) => a - b);
    const latest = repoRuns[0];
    return {
      url,
      name: repoShortName(url),
      routines: assigned
        .map((r) => ({ id: r.id, title: r.title }))
        .sort((a, b) => a.title.localeCompare(b.title)),
      total: assigned.length,
      enabled: assigned.filter((r) => r.enabled).length,
      runningNow: assigned.filter((r) => r.is_running).length,
      branches: [...new Set(entries.map((e) => e.branch?.trim() || "default"))].sort(),
      agents: [...new Set(assigned.map((r) => r.agent))].sort(),
      autoPullOff: assigned.filter((r) =>
        (r.repositories ?? []).some((e) => e.repository === url && e.auto_pull === false),
      ).length,
      failing: assigned.filter((r) => finished.find((run) => run.routine_id === r.id)?.status === "failed").length,
      successCount: finished.filter((run) => run.status === "success").length,
      finishedCount: finished.length,
      p50Secs: percentile(durations, 0.5),
      lastRun: latest ? { label: latest.started_at_local, status: latest.status } : null,
    };
  });
  return stats.sort(
    (a, b) => b.failing - a.failing || b.total - a.total || a.name.localeCompare(b.name),
  );
}

/** `successCount / finishedCount`, or `null` when this repository has no finished runs in sample. */
export function repoSuccessRate(r: RepoStats): number | null {
  return r.finishedCount === 0 ? null : r.successCount / r.finishedCount;
}

/** Count of routines that declare no repository at all. */
export function routinesWithoutRepo(routines: RoutineResponse[]): number {
  return routines.filter((r) => !r.repositories || r.repositories.length === 0).length;
}

/** Trimmed, case-insensitive substring match against a repository's short name or URL. */
export function matchesRepoQuery(r: RepoStats, query: string): boolean {
  const q = query.trim().toLowerCase();
  return q === "" || r.name.toLowerCase().includes(q) || r.url.toLowerCase().includes(q);
}

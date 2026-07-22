/**
 * Pure aggregation logic for the MACHINES page: joins the known machine names against every
 * routine's `machines` targeting list and the fleet-wide run feed to build a per-machine fleet
 * inventory — the ops-dashboard "runners/agents pool" view (CI/CD Insights, cron-manager host
 * lists) this dashboard didn't have: routine counts, live/power-saving state, recent success
 * rate, and last activity, all from data the UI already fetches for the Routines and Reliability
 * pages. No new API calls or backend fields.
 */
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { routineHealth } from "../routines/filter";

export interface MachineRoutineRef {
  id: string;
  title: string;
}

export interface MachineStats {
  name: string;
  isCurrent: boolean;
  routines: MachineRoutineRef[];
  total: number;
  enabled: number;
  runningNow: number;
  powerSaving: number;
  /** Routines targeting this machine whose agent is unregistered/unrunnable or schedule is dead. */
  needsAttention: number;
  successCount: number;
  finishedCount: number;
  lastRun: { label: string; status: FleetRunSummary["status"] } | null;
}

/** Per-machine fleet inventory, one entry per name in `machineNames`, in the given order. */
export function computeMachineStats(
  machineNames: string[],
  routines: RoutineResponse[],
  runs: FleetRunSummary[],
  currentMachineName: string | undefined,
  now: Date,
): MachineStats[] {
  return machineNames.map((name) => {
    const assigned = routines.filter((r) => (r.machines ?? []).includes(name));
    const ids = new Set(assigned.map((r) => r.id));
    const machineRuns = runs.filter((run) => ids.has(run.routine_id));
    const finished = machineRuns.filter((run) => run.status === "success" || run.status === "failed");
    const latest = machineRuns.reduce<FleetRunSummary | null>(
      (best, run) => (best === null || run.started_at > best.started_at ? run : best),
      null,
    );
    return {
      name,
      isCurrent: name === currentMachineName,
      routines: assigned
        .map((r) => ({ id: r.id, title: r.title }))
        .sort((a, b) => a.title.localeCompare(b.title)),
      total: assigned.length,
      enabled: assigned.filter((r) => r.enabled).length,
      runningNow: assigned.filter((r) => r.is_running).length,
      powerSaving: assigned.filter((r) => r.power_saving).length,
      needsAttention: assigned.filter((r) => {
        const h = routineHealth(r, now);
        return h === "agent-missing" || h === "dead-schedule";
      }).length,
      successCount: finished.filter((run) => run.status === "success").length,
      finishedCount: finished.length,
      lastRun: latest ? { label: latest.started_at_local, status: latest.status } : null,
    };
  });
}

/** `successCount / finishedCount`, or `null` when this machine has no finished runs in sample. */
export function machineSuccessRate(m: MachineStats): number | null {
  return m.finishedCount === 0 ? null : m.successCount / m.finishedCount;
}

/** Routines targeting no machine at all — dormant until assigned (mirrors the Routines "dormant" facet). */
export function unassignedRoutineCount(routines: RoutineResponse[]): number {
  return routines.filter((r) => (r.machines ?? []).every((m) => m.trim() === "")).length;
}

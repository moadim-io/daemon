/**
 * Pure aggregation logic for the AGENTS page: joins the `GET /agents` registry against every
 * routine's `agent` and the fleet-wide run feed to build a per-agent breakdown — the
 * "executor pool by label" view CI dashboards ship (GitHub Actions runner labels, Buildkite
 * agent queues), so one broken agent CLI shows up as one row instead of N failing routines.
 * No new API calls or backend fields.
 */
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { percentile } from "../reliability/reliabilityStats";

/** Worst-first: an agent the registry doesn't know outranks a missing binary, and so on. */
export type AgentHealth = "unregistered" | "cli-missing" | "setup-missing" | "ok" | "unused";

export interface AgentRoutineRef {
  id: string;
  title: string;
}

export interface AgentStats {
  name: string;
  health: AgentHealth;
  routines: AgentRoutineRef[];
  total: number;
  enabled: number;
  runningNow: number;
  /** Distinct non-blank `model` overrides, sorted; routines on the agent's default model add none. */
  models: string[];
  successCount: number;
  finishedCount: number;
  p50Secs: number | null;
  lastRun: { label: string; status: FleetRunSummary["status"] } | null;
}

const HEALTH_RANK: Record<AgentHealth, number> = {
  unregistered: 0,
  "cli-missing": 1,
  "setup-missing": 2,
  ok: 3,
  unused: 4,
};

function agentHealth(name: string, registered: Set<string>, assigned: RoutineResponse[]): AgentHealth {
  // ponytail: a name with no routines can only come from the registry, so it is registered.
  if (assigned.length === 0) return "unused";
  if (!registered.has(name) || assigned.some((r) => !r.agent_registered)) return "unregistered";
  if (assigned.some((r) => !r.agent_command_available)) return "cli-missing";
  if (assigned.some((r) => !r.agent_setup_available)) return "setup-missing";
  return "ok";
}

/**
 * One entry per agent in the union of `registeredAgents` and every routine's `agent`, ranked
 * unhealthy-first, then by routine count (busiest first), then name.
 */
export function computeAgentStats(
  registeredAgents: string[],
  routines: RoutineResponse[],
  runs: FleetRunSummary[],
): AgentStats[] {
  const registered = new Set(registeredAgents);
  const names = new Set([...registeredAgents, ...routines.map((r) => r.agent)]);
  const stats = [...names].map((name): AgentStats => {
    const assigned = routines.filter((r) => r.agent === name);
    const ids = new Set(assigned.map((r) => r.id));
    const agentRuns = runs.filter((run) => ids.has(run.routine_id));
    const finished = agentRuns.filter((run) => run.status === "success" || run.status === "failed");
    const durations = finished
      .flatMap((run) =>
        run.finished_at != null && run.finished_at >= run.started_at ? [run.finished_at - run.started_at] : [],
      )
      .sort((a, b) => a - b);
    const latest = agentRuns.reduce<FleetRunSummary | null>(
      (best, run) => (best === null || run.started_at > best.started_at ? run : best),
      null,
    );
    return {
      name,
      health: agentHealth(name, registered, assigned),
      routines: assigned
        .map((r) => ({ id: r.id, title: r.title }))
        .sort((a, b) => a.title.localeCompare(b.title)),
      total: assigned.length,
      enabled: assigned.filter((r) => r.enabled).length,
      runningNow: assigned.filter((r) => r.is_running).length,
      models: [...new Set(assigned.map((r) => r.model?.trim() ?? "").filter((m) => m !== ""))].sort(),
      successCount: finished.filter((run) => run.status === "success").length,
      finishedCount: finished.length,
      p50Secs: percentile(durations, 0.5),
      lastRun: latest ? { label: latest.started_at_local, status: latest.status } : null,
    };
  });
  return stats.sort(
    (a, b) => HEALTH_RANK[a.health] - HEALTH_RANK[b.health] || b.total - a.total || a.name.localeCompare(b.name),
  );
}

/** `successCount / finishedCount`, or `null` when this agent has no finished runs in sample. */
export function agentSuccessRate(a: AgentStats): number | null {
  return a.finishedCount === 0 ? null : a.successCount / a.finishedCount;
}

/** True for a health state that stops this agent's routines from running. */
export function isUnhealthy(health: AgentHealth): boolean {
  return health === "unregistered" || health === "cli-missing" || health === "setup-missing";
}

/** Badge text + `run-status` pill class for a health state. */
export function healthBadge(health: AgentHealth): { label: string; className: string; title: string } {
  switch (health) {
    case "unregistered":
      return { label: "UNREGISTERED", className: "run-status failed", title: "Not in this daemon's agent registry" };
    case "cli-missing":
      return { label: "CLI MISSING", className: "run-status failed", title: "The agent's command is not on the daemon PATH" };
    case "setup-missing":
      return { label: "SETUP MISSING", className: "run-status running", title: "The agent's setup tool is not on the daemon PATH" };
    case "ok":
      return { label: "OK", className: "run-status success", title: "Registered, and its command resolves on PATH" };
    case "unused":
      return { label: "UNUSED", className: "cell-meta", title: "Registered, but no routine targets it" };
  }
}

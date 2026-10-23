import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { agentSuccessRate, computeAgentStats, healthBadge, isUnhealthy, type AgentHealth } from "./agentsStats";

function routine(id: string, overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id,
    title: id,
    agent: "claude",
    model: null,
    schedule: "* * * * *",
    prompt: "",
    repositories: [],
    machines: ["box-1"],
    enabled: true,
    source: "",
    created_at: 0,
    updated_at: 0,
    last_manual_trigger_at: null,
    last_scheduled_trigger_at: null,
    snoozed_until: null,
    skip_runs: null,
    power_saving: false,
    ttl_secs: null,
    tags: [],
    agent_registered: true,
    agent_command_available: true,
    agent_setup_available: true,
    is_running: false,
    file_path: "",
    folder: null,
    slug: "routine",
    rel_path: "routine",
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

function run(routineId: string, startedAt: number, overrides: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: routineId,
    workbench: `wb-${startedAt}`,
    started_at: startedAt,
    started_at_local: `t${startedAt}`,
    status: "success",
    exit_code: 0,
    finished_at: startedAt + 10,
    finished_at_local: "",
    ...overrides,
  };
}

describe("computeAgentStats", () => {
  it("unions the registry with routine agents and rolls up routines, models, and runs", () => {
    const stats = computeAgentStats(
      ["claude", "codex"],
      [
        routine("r1", { agent: "claude", model: "opus", is_running: true }),
        routine("r2", { agent: "claude", model: " ", enabled: false }),
        routine("r3", { agent: "claude", model: "haiku" }),
      ],
      [
        run("r1", 100, { finished_at: 130 }),
        run("r3", 200, { status: "failed", finished_at: 210 }),
        run("r1", 300, { status: "running", finished_at: null }),
        run("r2", 50, { finished_at: 40 }),
        run("other", 999),
      ],
    );
    expect(stats.map((a) => a.name)).toEqual(["claude", "codex"]);
    const claude = stats[0]!;
    expect(claude).toMatchObject({
      health: "ok",
      total: 3,
      enabled: 2,
      runningNow: 1,
      models: ["haiku", "opus"],
      successCount: 2,
      finishedCount: 3,
      p50Secs: 10,
      lastRun: { label: "t300", status: "running" },
    });
    expect(claude.routines.map((r) => r.id)).toEqual(["r1", "r2", "r3"]);
    expect(agentSuccessRate(claude)).toBeCloseTo(2 / 3);
    expect(stats[1]).toMatchObject({ health: "unused", total: 0, p50Secs: null, lastRun: null });
    expect(agentSuccessRate(stats[1]!)).toBeNull();
  });

  it("ranks unhealthy agents first, then busiest, then by name", () => {
    const stats = computeAgentStats(
      ["a", "b", "c", "d", "e"],
      [
        routine("ghost", { agent: "zz" }),
        routine("off", { agent: "a", agent_registered: false }),
        routine("cli", { agent: "b", agent_command_available: false }),
        routine("setup", { agent: "c", agent_setup_available: false }),
        routine("ok1", { agent: "d" }),
        routine("ok2", { agent: "e" }),
        routine("ok3", { agent: "e" }),
      ],
      [],
    );
    expect(stats.map((a) => [a.name, a.health])).toEqual([
      ["a", "unregistered"],
      ["zz", "unregistered"],
      ["b", "cli-missing"],
      ["c", "setup-missing"],
      ["e", "ok"],
      ["d", "ok"],
    ]);
  });

  it("returns nothing for an empty registry and fleet", () => {
    expect(computeAgentStats([], [], [])).toEqual([]);
  });
});

describe("health helpers", () => {
  it("flags only states that block runs", () => {
    const states: AgentHealth[] = ["unregistered", "cli-missing", "setup-missing", "ok", "unused"];
    expect(states.filter(isUnhealthy)).toEqual(["unregistered", "cli-missing", "setup-missing"]);
    expect(states.map((h) => healthBadge(h).label)).toEqual([
      "UNREGISTERED",
      "CLI MISSING",
      "SETUP MISSING",
      "OK",
      "UNUSED",
    ]);
  });
});

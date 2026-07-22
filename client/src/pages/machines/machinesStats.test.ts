import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { computeMachineStats, machineSuccessRate, unassignedRoutineCount, type MachineStats } from "./machinesStats";

/** `computeMachineStats` always returns one entry per input name, in order — index safely. */
function at(stats: MachineStats[], i: number): MachineStats {
  const m = stats[i];
  if (!m) throw new Error(`expected a MachineStats at index ${i}`);
  return m;
}

const NOW = new Date("2026-07-22T12:00:00Z");

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
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

function run(routineId: string, overrides: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: routineId,
    workbench: `${routineId}-wb`,
    started_at: 100,
    started_at_local: "12:00",
    status: "success",
    exit_code: 0,
    finished_at: 110,
    finished_at_local: "12:01",
    ...overrides,
  };
}

describe("computeMachineStats", () => {
  it("counts routines, enabled, running, power-saving, and attention per machine", () => {
    const routines = [
      routine("r1", { machines: ["box-1"], enabled: true, is_running: true }),
      routine("r2", { machines: ["box-1"], enabled: false, power_saving: true }),
      routine("r3", { machines: ["box-2"], agent_registered: false }),
      routine("r4", { machines: [] }),
    ];
    const stats = computeMachineStats(["box-1", "box-2"], routines, [], "box-1", NOW);
    const box1 = at(stats, 0);
    const box2 = at(stats, 1);

    expect(box1.total).toBe(2);
    expect(box1.enabled).toBe(1);
    expect(box1.runningNow).toBe(1);
    expect(box1.powerSaving).toBe(1);
    expect(box1.needsAttention).toBe(0);
    expect(box1.isCurrent).toBe(true);
    expect(box1.routines.map((r) => r.id)).toEqual(["r1", "r2"]);

    expect(box2.total).toBe(1);
    expect(box2.needsAttention).toBe(1);
    expect(box2.isCurrent).toBe(false);
  });

  it("joins runs to a machine via its routines' ids and picks the most recent as lastRun", () => {
    const routines = [routine("r1", { machines: ["box-1"] }), routine("r2", { machines: ["box-2"] })];
    const runs = [
      run("r1", { started_at: 100, status: "success", started_at_local: "first" }),
      run("r1", { started_at: 200, status: "failed", started_at_local: "second" }),
      run("r2", { started_at: 999, status: "success" }),
    ];
    const box1 = at(computeMachineStats(["box-1", "box-2"], routines, runs, undefined, NOW), 0);

    expect(box1.lastRun).toEqual({ label: "second", status: "failed" });
    expect(box1.finishedCount).toBe(2);
    expect(box1.successCount).toBe(1);
  });

  it("excludes still-running runs from the finished sample and reports null lastRun with no runs", () => {
    const routines = [routine("r1", { machines: ["box-1"] })];
    const runs = [run("r1", { status: "running", finished_at: null })];
    const box1 = at(computeMachineStats(["box-1"], routines, runs, undefined, NOW), 0);
    expect(box1.finishedCount).toBe(0);
    expect(box1.lastRun).toEqual({ label: expect.any(String), status: "running" });

    const empty = at(computeMachineStats(["box-2"], routines, runs, undefined, NOW), 0);
    expect(empty.lastRun).toBeNull();
  });

  it("sorts a machine's routines by title", () => {
    const routines = [
      routine("r1", { title: "Zeta", machines: ["box-1"] }),
      routine("r2", { title: "Alpha", machines: ["box-1"] }),
    ];
    const box1 = at(computeMachineStats(["box-1"], routines, [], undefined, NOW), 0);
    expect(box1.routines.map((r) => r.title)).toEqual(["Alpha", "Zeta"]);
  });
});

describe("machineSuccessRate", () => {
  it("is null with no finished runs, else successCount / finishedCount", () => {
    const routines = [routine("r1", { machines: ["box-1"] })];
    const noRuns = at(computeMachineStats(["box-1"], routines, [], undefined, NOW), 0);
    expect(machineSuccessRate(noRuns)).toBeNull();

    const runs = [run("r1", { status: "success" }), run("r1", { status: "failed" })];
    const withRuns = at(computeMachineStats(["box-1"], routines, runs, undefined, NOW), 0);
    expect(machineSuccessRate(withRuns)).toBe(0.5);
  });
});

describe("unassignedRoutineCount", () => {
  it("counts routines whose machines list is empty or blank-only", () => {
    const routines = [
      routine("r1", { machines: [] }),
      routine("r2", { machines: [""] }),
      routine("r3", { machines: ["box-1"] }),
    ];
    expect(unassignedRoutineCount(routines)).toBe(2);
  });
});

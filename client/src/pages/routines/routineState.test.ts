// Ported 1:1 from the sort/group portions of ui/src/routines/state_tests.rs (removed) and the whole of
// ui/src/routines/state_group_by_tests.rs (removed). (The Yew-reducer selection/modal tests in
// state_tests.rs don't port 1:1 — this page uses plain React state instead of a Yew reducer; that
// behavior is covered by RoutinesPage's own tests instead.)
import { describe, expect, it } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import {
  flipDir,
  groupRoutines,
  parseRGroupBy,
  routineGroupKey,
  sortRoutines,
  type RDir,
} from "./routineState";

function routine(
  id: string,
  title: string,
  agent: string,
  schedule: string,
  machines: string[],
  enabled: boolean,
  overrides: Partial<RoutineResponse> = {},
): RoutineResponse {
  return {
    id,
    title,
    agent,
    model: null,
    schedule,
    prompt: "",
    repositories: [],
    machines,
    enabled,
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
    agent_registered: false,
    agent_command_available: false,
    // Defaults to available so the `agent_registered`-only overrides below (e.g.
    // `routineWithHealth`) keep exercising the health state they were written for.
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

function now(): Date {
  return new Date(2026, 0, 1, 12, 0, 0);
}

describe("routineState — sort_routines", () => {
  function routineSort(id: string, title: string, agent: string, enabled: boolean, updatedAt: number) {
    return routine(id, title, agent, "0 * * * *", [], enabled, { updated_at: updatedAt });
  }

  it("RDir.flip toggles direction", () => {
    expect(flipDir("asc" as RDir)).toBe("desc");
    expect(flipDir("desc" as RDir)).toBe("asc");
  });

  it("none col preserves insertion order", () => {
    const rs = [routineSort("z", "Zebra", "claude", true, 10), routineSort("a", "Alpha", "codex", true, 5)];
    const sorted = sortRoutines(rs, undefined, "asc", now());
    expect(sorted[0]?.id).toBe("z");
    expect(sorted[1]?.id).toBe("a");
  });

  it("by title ascending", () => {
    const rs = [
      routineSort("b", "Zebra", "claude", true, 10),
      routineSort("a", "Alpha", "claude", true, 5),
      routineSort("c", "Mango", "claude", true, 7),
    ];
    const sorted = sortRoutines(rs, "title", "asc", now());
    expect(sorted.map((r) => r.title)).toEqual(["Alpha", "Mango", "Zebra"]);
  });

  it("by title descending", () => {
    const rs = [
      routineSort("b", "Zebra", "claude", true, 10),
      routineSort("a", "Alpha", "claude", true, 5),
      routineSort("c", "Mango", "claude", true, 7),
    ];
    const sorted = sortRoutines(rs, "title", "desc", now());
    expect(sorted.map((r) => r.title)).toEqual(["Zebra", "Mango", "Alpha"]);
  });

  it("by agent ascending", () => {
    const rs = [routineSort("a", "T1", "codex", true, 1), routineSort("b", "T2", "claude", true, 2)];
    const sorted = sortRoutines(rs, "agent", "asc", now());
    expect(sorted.map((r) => r.agent)).toEqual(["claude", "codex"]);
  });

  it("by updated ascending", () => {
    const rs = [
      routineSort("a", "T1", "claude", true, 100),
      routineSort("b", "T2", "claude", true, 50),
      routineSort("c", "T3", "claude", true, 75),
    ];
    const sorted = sortRoutines(rs, "updated", "asc", now());
    expect(sorted.map((r) => r.id)).toEqual(["b", "c", "a"]);
  });

  it("by updated descending", () => {
    const rs = [
      routineSort("a", "T1", "claude", true, 100),
      routineSort("b", "T2", "claude", true, 50),
      routineSort("c", "T3", "claude", true, 75),
    ];
    const sorted = sortRoutines(rs, "updated", "desc", now());
    expect(sorted.map((r) => r.id)).toEqual(["a", "c", "b"]);
  });

  it("by enabled puts disabled first ascending", () => {
    const rs = [
      routineSort("a", "T1", "claude", true, 1),
      routineSort("b", "T2", "claude", false, 2),
      routineSort("c", "T3", "claude", true, 3),
    ];
    const sorted = sortRoutines(rs, "enabled", "asc", now());
    expect(sorted[0]?.enabled).toBe(false);
    expect(sorted[1]?.enabled).toBe(true);
    expect(sorted[2]?.enabled).toBe(true);
  });

  it("by next_run puts none after some", () => {
    const rs = [
      routineSort("dis", "Disabled", "claude", false, 1),
      routineSort("hourly", "Hourly", "claude", true, 2),
    ];
    const sorted = sortRoutines(rs, "next_run", "asc", now());
    expect(sorted.map((r) => r.id)).toEqual(["hourly", "dis"]);
  });

  it("title sort is case insensitive", () => {
    const rs = [routineSort("a", "zebra", "claude", true, 1), routineSort("b", "ALPHA", "claude", true, 2)];
    const sorted = sortRoutines(rs, "title", "asc", now());
    expect(sorted.map((r) => r.title)).toEqual(["ALPHA", "zebra"]);
  });

  function routineWithHealth(id: string, enabled: boolean, machines: string[], agentRegistered: boolean) {
    return routine(id, id, "claude", "0 * * * *", machines, enabled, { agent_registered: agentRegistered });
  }

  it("by health ascending puts most broken first", () => {
    const rs = [
      routineWithHealth("healthy", true, ["m1"], true),
      routineWithHealth("dormant", true, [], true),
      routineWithHealth("disabled", false, ["m1"], false),
    ];
    const sorted = sortRoutines(rs, "health", "asc", now());
    expect(sorted.map((r) => r.id)).toEqual(["dormant", "disabled", "healthy"]);
  });

  it("by health descending puts healthy first", () => {
    const rs = [routineWithHealth("dormant", true, [], true), routineWithHealth("healthy", true, ["m1"], true)];
    const sorted = sortRoutines(rs, "health", "desc", now());
    expect(sorted.map((r) => r.id)).toEqual(["healthy", "dormant"]);
  });

  function routineWithLastFire(id: string, manual: number | null, scheduled: number | null) {
    return routine(id, id, "claude", "0 * * * *", [], true, {
      last_manual_trigger_at: manual,
      last_scheduled_trigger_at: scheduled,
    });
  }

  it("by last_fire ascending puts oldest first", () => {
    const rs = [
      routineWithLastFire("new", 300, null),
      routineWithLastFire("old", 100, null),
      routineWithLastFire("never", null, null),
    ];
    const sorted = sortRoutines(rs, "last_fire", "asc", now());
    expect(sorted.map((r) => r.id)).toEqual(["never", "old", "new"]);
  });

  it("by last_fire descending puts newest first", () => {
    const rs = [routineWithLastFire("old", 100, null), routineWithLastFire("new", 300, null)];
    const sorted = sortRoutines(rs, "last_fire", "desc", now());
    expect(sorted.map((r) => r.id)).toEqual(["new", "old"]);
  });
});

describe("routineState — group by (state_group_by_tests.rs)", () => {
  it("RGroupBy default is none for unknown token", () => {
    expect(parseRGroupBy("bogus")).toBe("none");
    expect(parseRGroupBy("")).toBe("none");
  });

  it("routine_group_key agent returns agent field", () => {
    const r = routine("id1", "t", "claude", "0 * * * *", ["m1"], true);
    expect(routineGroupKey(r, "agent")).toBe("claude");
  });

  it("routine_group_key machine returns first machine", () => {
    const r = routine("id1", "t", "claude", "0 * * * *", ["alpha", "beta"], true);
    expect(routineGroupKey(r, "machine")).toBe("alpha");
  });

  it("routine_group_key machine returns unassigned when no machines", () => {
    const r = routine("id1", "t", "claude", "0 * * * *", [], true);
    expect(routineGroupKey(r, "machine")).toBe("(unassigned)");
  });

  it("routine_group_key status enabled", () => {
    const r = routine("id1", "t", "claude", "0 * * * *", [], true);
    expect(routineGroupKey(r, "status")).toBe("Enabled");
  });

  it("routine_group_key status disabled", () => {
    const r = routine("id1", "t", "claude", "0 * * * *", [], false);
    expect(routineGroupKey(r, "status")).toBe("Disabled");
  });

  it("routine_group_key none returns empty string", () => {
    const r = routine("id1", "t", "claude", "0 * * * *", [], true);
    expect(routineGroupKey(r, "none")).toBe("");
  });

  it("group_routines none returns single group with all routines", () => {
    const rs = [routine("a", "t", "claude", "0 * * * *", [], true), routine("b", "t", "codex", "0 * * * *", [], false)];
    const groups = groupRoutines(rs, "none");
    expect(groups.length).toBe(1);
    expect(groups[0]?.[0]).toBe("");
    expect(groups[0]?.[1].length).toBe(2);
  });

  it("group_routines by agent creates one group per agent", () => {
    const rs = [
      routine("a", "t", "claude", "0 * * * *", [], true),
      routine("b", "t", "codex", "0 * * * *", [], true),
      routine("c", "t", "claude", "0 * * * *", [], true),
    ];
    const groups = groupRoutines(rs, "agent");
    expect(groups.length).toBe(2);
    expect(groups[0]?.[0]).toBe("claude");
    expect(groups[0]?.[1].length).toBe(2);
    expect(groups[1]?.[0]).toBe("codex");
    expect(groups[1]?.[1].length).toBe(1);
  });

  it("group_routines by agent preserves input order within group", () => {
    const rs = [
      routine("first", "t", "claude", "0 * * * *", [], true),
      routine("second", "t", "claude", "0 * * * *", [], true),
    ];
    const groups = groupRoutines(rs, "agent");
    expect(groups[0]?.[1][0]?.id).toBe("first");
    expect(groups[0]?.[1][1]?.id).toBe("second");
  });

  it("group_routines by machine separates unassigned", () => {
    const rs = [
      routine("a", "t", "claude", "0 * * * *", ["worker-1"], true),
      routine("b", "t", "claude", "0 * * * *", [], true),
      routine("c", "t", "claude", "0 * * * *", ["worker-1"], true),
    ];
    const groups = groupRoutines(rs, "machine");
    expect(groups.length).toBe(2);
    expect(groups[0]?.[0]).toBe("(unassigned)");
    expect(groups[0]?.[1].length).toBe(1);
    expect(groups[1]?.[0]).toBe("worker-1");
    expect(groups[1]?.[1].length).toBe(2);
  });

  it("group_routines by status splits enabled and disabled", () => {
    const rs = [
      routine("a", "t", "claude", "0 * * * *", [], true),
      routine("b", "t", "claude", "0 * * * *", [], false),
      routine("c", "t", "claude", "0 * * * *", [], true),
    ];
    const groups = groupRoutines(rs, "status");
    expect(groups.length).toBe(2);
    expect(groups[0]?.[0]).toBe("Disabled");
    expect(groups[0]?.[1].length).toBe(1);
    expect(groups[1]?.[0]).toBe("Enabled");
    expect(groups[1]?.[1].length).toBe(2);
  });
});

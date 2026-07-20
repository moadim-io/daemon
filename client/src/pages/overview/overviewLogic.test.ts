import { describe, expect, it } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import {
  attentionItems,
  attentionReason,
  computeKpis,
  fromRoutine,
  nextRunSummary,
  sourcesOf,
  upcomingRuns,
  UPCOMING_LIMIT,
  type Kind,
  type SchedSource,
} from "./overviewLogic";

/** Fixed reference instant (10:00 local, Mon 2026-06-22) so cron math is deterministic. */
const atTen = () => new Date(2026, 5, 22, 10, 0, 0);

/** A healthy source: targets a machine and a registered agent. Mutate the return to opt into a fault. */
function src(kind: Kind, label: string, schedule: string, enabled: boolean): SchedSource {
  return {
    kind,
    id: label,
    label,
    schedule,
    human: undefined,
    enabled,
    machinesEmpty: false,
    agentRegistered: true,
    flagCount: 0,
    snoozed: false,
  };
}

/** Minimal valid `RoutineResponse` fixture; override only the fields a test cares about. */
function makeRoutine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "r1",
    schedule: "0 0 * * *",
    title: "T",
    agent: "claude",
    prompt: "p",
    source: "managed",
    created_at: 0,
    updated_at: 0,
    enabled: true,
    machines: [],
    agent_registered: false,
    agent_command_available: false,
    file_path: "",
    flag_count: 0,
    env_keys: [],
    is_running: false,
    schedule_description: null,
    snoozed_until: null,
    skip_runs: null,
    ...overrides,
  };
}

describe("computeKpis", () => {
  it("counts total/enabled/disabled/due-soon", () => {
    const sources = [
      src("routine", "a", "*/5 * * * *", true), // enabled, fires in 5m -> due soon
      src("routine", "b", "0 0 * * *", true), // enabled, fires at midnight -> far
      src("routine", "c", "*/5 * * * *", false), // disabled -> never due
    ];
    const kpis = computeKpis(sources, atTen());
    expect(kpis.total).toBe(3);
    expect(kpis.enabled).toBe(2);
    expect(kpis.disabled).toBe(1);
    expect(kpis.dueSoon).toBe(1);
  });

  it("due-soon excludes snoozed", () => {
    const a = { ...src("routine", "a", "*/5 * * * *", true), snoozed: true }; // would fire in 5m but snoozed
    const b = src("routine", "b", "*/5 * * * *", true); // enabled + not snoozed -> due
    const kpis = computeKpis([a, b], atTen());
    expect(kpis.dueSoon).toBe(1);
  });

  it("is all-zero for an empty fleet", () => {
    const kpis = computeKpis([], atTen());
    expect(kpis).toEqual({
      total: 0,
      enabled: 0,
      disabled: 0,
      dueSoon: 0,
      attention: 0,
      flags: 0,
      snoozed: 0,
      dormant: 0,
    });
  });

  it("sums flags across all sources", () => {
    const a = { ...src("routine", "a", "*/5 * * * *", true), flagCount: 3 };
    const b = { ...src("routine", "b", "0 0 * * *", false), flagCount: 1 };
    const c = src("routine", "c", "*/5 * * * *", true);
    const kpis = computeKpis([a, b, c], atTen());
    expect(kpis.flags).toBe(4);
  });

  it("flags is zero when no flags", () => {
    const sources = [src("routine", "a", "*/5 * * * *", true), src("routine", "b", "0 0 * * *", true)];
    expect(computeKpis(sources, atTen()).flags).toBe(0);
  });

  it("counts only enabled snoozed sources", () => {
    const a = { ...src("routine", "a", "*/5 * * * *", true), snoozed: true };
    const b = { ...src("routine", "b", "0 0 * * *", false), snoozed: true }; // disabled -> not counted
    const c = src("routine", "c", "*/5 * * * *", true);
    const kpis = computeKpis([a, b, c], atTen());
    expect(kpis.snoozed).toBe(1);
  });

  it("counts enabled sources with no machines as dormant", () => {
    const dormant = { ...src("routine", "d", "*/5 * * * *", true), machinesEmpty: true };
    const active = src("routine", "a", "*/5 * * * *", true); // has machines
    const disabledNoMachine = { ...src("routine", "c", "*/5 * * * *", false), machinesEmpty: true }; // disabled
    const kpis = computeKpis([dormant, active, disabledNoMachine], atTen());
    expect(kpis.dormant).toBe(1);
  });

  it("snoozed is zero when none snoozed", () => {
    const sources = [src("routine", "a", "*/5 * * * *", true), src("routine", "b", "0 0 * * *", true)];
    expect(computeKpis(sources, atTen()).snoozed).toBe(0);
  });

  it("counts attention", () => {
    const dormant = { ...src("routine", "d", "*/5 * * * *", true), machinesEmpty: true };
    const sources = [
      dormant,
      src("routine", "ok", "*/5 * * * *", true),
      src("routine", "off", "not a cron", false), // disabled -> not counted
    ];
    expect(computeKpis(sources, atTen()).attention).toBe(1);
  });
});

describe("upcomingRuns", () => {
  it("sorts soonest first, excludes disabled and invalid", () => {
    const sources = [
      src("routine", "midnight", "0 0 * * *", true),
      src("routine", "five", "*/5 * * * *", true),
      src("routine", "off", "*/1 * * * *", false), // disabled -> excluded
      src("routine", "bad", "not a cron", true), // invalid -> excluded
    ];
    const runs = upcomingRuns(sources, atTen());
    expect(runs).toHaveLength(2);
    expect(runs[0]?.label).toBe("five"); // 10:05 sorts before midnight
    expect(runs[0]?.kind).toBe("routine");
    expect(runs[1]?.label).toBe("midnight");
    expect(runs[0]?.soon).toBe(true);
    expect(runs[1]?.soon).toBe(false);
  });

  it("excludes snoozed sources", () => {
    const a = { ...src("routine", "snoozed", "*/5 * * * *", true), snoozed: true };
    const b = src("routine", "active", "*/5 * * * *", true);
    const runs = upcomingRuns([a, b], atTen());
    expect(runs).toHaveLength(1);
    expect(runs[0]?.label).toBe("active");
  });

  it("truncates to the limit", () => {
    const sources = Array.from({ length: UPCOMING_LIMIT + 4 }, (_, i) =>
      src("routine", `job${String(i).padStart(2, "0")}`, "*/5 * * * *", true),
    );
    expect(upcomingRuns(sources, atTen())).toHaveLength(UPCOMING_LIMIT);
  });

  it("breaks ties by label", () => {
    const sources = [src("routine", "zeta", "*/5 * * * *", true), src("routine", "alpha", "*/5 * * * *", true)];
    const runs = upcomingRuns(sources, atTen());
    expect(runs[0]?.label).toBe("alpha");
    expect(runs[1]?.label).toBe("zeta");
  });

  it("preserves the human schedule description", () => {
    const source = { ...src("routine", "nightly", "0 0 * * *", true), human: "At midnight" };
    const runs = upcomingRuns([source], atTen());
    expect(runs[0]?.human).toBe("At midnight");
  });

  it("carries the source id, distinct from label", () => {
    const source = src("routine", "daily-backup", "*/5 * * * *", true);
    const runs = upcomingRuns([source], atTen());
    expect(runs[0]?.id).toBe("daily-backup");
    expect(runs[0]?.label).toBe("daily-backup");
  });

  it("carries the raw schedule string", () => {
    const runs = upcomingRuns([src("routine", "r", "*/15 * * * *", true)], atTen());
    expect(runs[0]?.schedule).toBe("*/15 * * * *");
  });

  it("carries the flag count from the source", () => {
    const source = { ...src("routine", "flagged", "*/5 * * * *", true), flagCount: 4 };
    expect(upcomingRuns([source], atTen())[0]?.flagCount).toBe(4);
  });

  it("flag count is zero when none", () => {
    const source = src("routine", "clean", "*/5 * * * *", true);
    expect(upcomingRuns([source], atTen())[0]?.flagCount).toBe(0);
  });
});

describe("nextRunSummary", () => {
  it("is the first run's countdown, or undefined", () => {
    const now = atTen();
    expect(nextRunSummary([], now)).toBeUndefined();
    const runs = upcomingRuns([src("routine", "five", "*/5 * * * *", true)], now);
    expect(nextRunSummary(runs, now)).toBe("in 5m");
  });
});

describe("fromRoutine", () => {
  it("uses title as label", () => {
    const routine = makeRoutine({ id: "r1", schedule: "0 0 * * *", title: "Nightly sweep", enabled: false });
    const source = fromRoutine(routine, atTen());
    expect(source.kind).toBe("routine");
    expect(source.id).toBe("r1");
    expect(source.label).toBe("Nightly sweep");
    expect(source.schedule).toBe("0 0 * * *");
    expect(source.enabled).toBe(false);
  });

  it("carries agent registration and machines", () => {
    const routine = makeRoutine({
      id: "r1",
      machines: ["box-1"],
      enabled: true,
      agent_registered: false,
    });
    const s = fromRoutine(routine, atTen());
    expect(s.agentRegistered).toBe(false);
    expect(s.machinesEmpty).toBe(false);
  });

  it("id differs from label", () => {
    const routine = makeRoutine({ id: "abc-uuid-123", schedule: "*/5 * * * *", title: "My Routine", enabled: true });
    const source = fromRoutine(routine, atTen());
    expect(source.id).toBe("abc-uuid-123");
    expect(source.label).toBe("My Routine");
    const runs = upcomingRuns([source], atTen());
    expect(runs[0]?.id).toBe("abc-uuid-123");
    expect(runs[0]?.label).toBe("My Routine");
  });

  /**
   * `snoozed` is derived from the caller-supplied `now`, not the real wall
   * clock — this is what makes `fromRoutine` host-testable/deterministic.
   */
  it("snoozed_until respects the passed-in now", () => {
    const routine = makeRoutine({
      id: "r1",
      enabled: true,
      snoozed_until: Math.floor(atTen().getTime() / 1000) + 60,
    });
    expect(fromRoutine(routine, atTen()).snoozed).toBe(true);
    const later = new Date(atTen().getTime() + 3_600_000);
    expect(fromRoutine(routine, later).snoozed).toBe(false);
  });
});

describe("sourcesOf", () => {
  it("maps routines", () => {
    const routine = makeRoutine({ id: "r1", schedule: "0 0 * * *", title: "T", enabled: true });
    const sources = sourcesOf([routine], atTen());
    expect(sources).toHaveLength(1);
    expect(sources[0]?.kind).toBe("routine");
    expect(sources[0]?.label).toBe("T");
  });
});

// ── NEEDS ATTENTION triage ──────────────────────────────────────────────────

describe("attentionReason", () => {
  it("skips disabled even when broken", () => {
    // A disabled entity is intentional, never flagged — even with every fault.
    const s = { ...src("routine", "off", "not a cron", false), machinesEmpty: true, agentRegistered: false };
    expect(attentionReason(s, atTen())).toBeUndefined();
  });

  it("healthy is undefined", () => {
    const s = src("routine", "ok", "*/5 * * * *", true);
    expect(attentionReason(s, atTen())).toBeUndefined();
  });

  it("dormant outranks other faults", () => {
    // No machine + dead schedule + missing agent -> dormant wins (highest priority).
    const s = { ...src("routine", "r", "not a cron", true), machinesEmpty: true, agentRegistered: false };
    expect(attentionReason(s, atTen())).toBe("dormant");
  });

  it("dead-schedule when no future fire", () => {
    // Has a machine, but the expression never parses -> no future fire.
    const s = src("routine", "c", "not a cron", true);
    expect(attentionReason(s, atTen())).toBe("dead-schedule");
  });

  it("agent-unregistered only when the schedule lives", () => {
    const s = { ...src("routine", "r", "*/5 * * * *", true), agentRegistered: false };
    expect(attentionReason(s, atTen())).toBe("agent-unregistered");
  });

  it("no agent fault when registered", () => {
    const s = src("routine", "c", "*/5 * * * *", true);
    expect(attentionReason(s, atTen())).toBeUndefined();
  });

  it("open flags surfaces when healthy otherwise", () => {
    const s = { ...src("routine", "r", "*/5 * * * *", true), flagCount: 2 };
    expect(attentionReason(s, atTen())).toBe("has-open-flags");
  });

  it("config faults outrank flags", () => {
    const s = { ...src("routine", "r", "*/5 * * * *", true), machinesEmpty: true, flagCount: 3 };
    // Dormant outranks has-open-flags.
    expect(attentionReason(s, atTen())).toBe("dormant");
  });

  it("disabled with flags is undefined", () => {
    const s = { ...src("routine", "r", "*/5 * * * *", false), flagCount: 5 };
    expect(attentionReason(s, atTen())).toBeUndefined();
  });
});

describe("attentionItems", () => {
  it("sorts by rank then label", () => {
    const dead = { ...src("routine", "zeta-dead", "not a cron", true), machinesEmpty: false };
    const dormantZ = { ...src("routine", "zeta-dormant", "*/5 * * * *", true), machinesEmpty: true };
    const dormantA = { ...src("routine", "alpha-dormant", "*/5 * * * *", true), machinesEmpty: true };
    const agent = { ...src("routine", "agent-missing", "*/5 * * * *", true), agentRegistered: false };
    const healthy = src("routine", "fine", "*/5 * * * *", true);

    const items = attentionItems([dead, dormantZ, dormantA, agent, healthy], atTen());
    // Healthy one excluded; dormant (rank 0) first, ties by label, then dead, then agent.
    expect(items).toHaveLength(4);
    expect(items[0]?.reason).toBe("dormant");
    expect(items[0]?.label).toBe("alpha-dormant");
    expect(items[1]?.reason).toBe("dormant");
    expect(items[1]?.label).toBe("zeta-dormant");
    expect(items[2]?.reason).toBe("dead-schedule");
    expect(items[3]?.reason).toBe("agent-unregistered");
  });

  it("carries the flag count for the has-open-flags reason", () => {
    const s = { ...src("routine", "flagged", "*/5 * * * *", true), flagCount: 7 };
    const items = attentionItems([s], atTen());
    expect(items).toHaveLength(1);
    expect(items[0]?.reason).toBe("has-open-flags");
    expect(items[0]?.flagCount).toBe(7);
  });

  it("is empty for a healthy fleet", () => {
    const items = attentionItems(
      [src("routine", "a", "*/5 * * * *", true), src("routine", "b", "0 0 * * *", true)],
      atTen(),
    );
    expect(items).toHaveLength(0);
  });
});

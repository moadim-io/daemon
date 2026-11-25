import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import {
  buildUsage,
  changeClass,
  changeLabel,
  fmtAgentTime,
  groupByFromParam,
  runSecs,
  usageDayLabel,
} from "./usageMath";

const NOW = new Date(2026, 10, 25, 12).getTime() / 1000;
const TODAY = new Date(2026, 10, 25).getTime() / 1000;
const YESTERDAY = new Date(2026, 10, 24).getTime() / 1000;
const TEN_DAYS_AGO = new Date(2026, 10, 15).getTime() / 1000;
const TWENTY_DAYS_AGO = new Date(2026, 10, 5).getTime() / 1000;

function run(
  routineId: string,
  startedAt: number,
  secs: number | null,
  status: FleetRunSummary["status"] = "success",
  title = routineId,
): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: title,
    workbench: `wb-${routineId}-${startedAt}`,
    started_at: startedAt,
    started_at_local: "",
    status,
    exit_code: null,
    finished_at: secs === null ? null : startedAt + secs,
    finished_at_local: null,
  };
}

const routines = [
  { id: "alpha", title: "Alpha", agent: "claude" },
  { id: "beta", title: "Beta", agent: "codex" },
] as RoutineResponse[];

const runs = [
  run("alpha", TODAY + 100, 600),
  run("alpha", YESTERDAY + 100, 300, "failed"),
  run("beta", NOW - 120, null, "running"),
  run("alpha", TEN_DAYS_AGO + 100, 1200),
  run("gone", TODAY + 200, 60, "success", "Gone"),
  run("alpha", TWENTY_DAYS_AGO, 999),
];

describe("groupByFromParam", () => {
  it("accepts agent and defaults to routine", () => {
    expect(groupByFromParam("agent")).toBe("agent");
    expect(groupByFromParam(null)).toBe("routine");
    expect(groupByFromParam("machine")).toBe("routine");
  });
});

describe("runSecs", () => {
  it("measures finished, running and unknown runs", () => {
    expect(runSecs(run("a", 100, 50), NOW)).toBe(50);
    expect(runSecs(run("a", NOW - 30, null, "running"), NOW)).toBe(30);
    expect(runSecs(run("a", 100, null, "unknown"), NOW)).toBe(0);
    expect(runSecs({ ...run("a", 100, 0), finished_at: 50 }, NOW)).toBe(0);
  });
});

describe("buildUsage", () => {
  it("sums agent time per day and per routine, with the previous period", () => {
    const r = buildUsage(runs, routines, NOW, 7, "routine", 2000);
    expect(r.days).toHaveLength(7);
    expect(r.totalSecs).toBe(1080);
    expect(r.prevTotalSecs).toBe(1200);
    expect(r.runs).toBe(4);
    const today = r.days[6]!;
    expect(today.successSecs).toBe(660);
    expect(today.otherSecs).toBe(120);
    expect(r.days[5]!.failedSecs).toBe(300);
    expect(r.rows.map((row) => [row.label, row.secs, row.runs, row.prevSecs])).toEqual([
      ["Alpha", 900, 2, 1200],
      ["Beta", 120, 1, 0],
      ["Gone", 60, 1, 0],
    ]);
    expect(r.partialSample).toBe(false);
  });

  it("groups by agent, with unknown for deleted routines", () => {
    const r = buildUsage(runs, routines, NOW, 7, "agent", 2000);
    expect(r.rows.map((row) => [row.key, row.secs, row.prevSecs])).toEqual([
      ["claude", 900, 1200],
      ["codex", 120, 0],
      ["unknown", 60, 0],
    ]);
  });

  it("flags a capped sample that does not reach the previous period", () => {
    expect(buildUsage([run("alpha", TODAY, 10)], routines, NOW, 7, "routine", 1).partialSample).toBe(true);
    expect(buildUsage(runs, routines, NOW, 7, "routine", runs.length).partialSample).toBe(false);
    expect(buildUsage([], routines, NOW, 7, "routine", 0).partialSample).toBe(false);
  });

  it("orders ties by previous time, then label", () => {
    const r = buildUsage(
      [run("b", TEN_DAYS_AGO, 50), run("b", TODAY, 10), run("a", TODAY, 10), run("c", TODAY, 10)],
      [],
      NOW,
      7,
      "routine",
      2000,
    );
    expect(r.rows.map((row) => row.key)).toEqual(["b", "a", "c"]);
  });
});

describe("formatting", () => {
  it("formats agent time", () => {
    expect(fmtAgentTime(59)).toBe("0m");
    expect(fmtAgentTime(2520)).toBe("42m");
    expect(fmtAgentTime(3 * 3600 + 300)).toBe("3h 5m");
    expect(fmtAgentTime(212 * 3600)).toBe("212h");
  });

  it("labels and colours the change", () => {
    expect(changeLabel(0, 0)).toBe("—");
    expect(changeLabel(5, 0)).toBe("new");
    expect(changeLabel(150, 100)).toBe("+50%");
    expect(changeLabel(50, 100)).toBe("-50%");
    expect(changeClass(120, 100)).toBe("c-red");
    expect(changeClass(105, 100)).toBeUndefined();
    expect(changeClass(5, 0)).toBeUndefined();
  });

  it("labels a day bar", () => {
    const day = { from: TODAY, to: TODAY + 86_400, successSecs: 0, failedSecs: 0, otherSecs: 0 };
    expect(usageDayLabel(day)).toMatch(/no agent time$/);
    expect(usageDayLabel({ ...day, successSecs: 3000, failedSecs: 600 })).toMatch(
      /1h 0m \(50m succeeded, 10m failed, 0m other\)$/,
    );
  });
});

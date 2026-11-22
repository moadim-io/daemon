import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { buildUptime, dayBounds, dayLabel, rangeFromParam, uptimeLabel, type DayCell } from "./uptimeMath";

const NOW = new Date(2026, 10, 16, 12).getTime() / 1000;
const TODAY = new Date(2026, 10, 16).getTime() / 1000;
const YESTERDAY = new Date(2026, 10, 15).getTime() / 1000;

function run(
  routineId: string,
  startedAt: number,
  status: FleetRunSummary["status"] = "success",
  title = routineId,
): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: title,
    workbench: `wb-${startedAt}`,
    started_at: startedAt,
    started_at_local: "",
    status,
    exit_code: null,
    finished_at: null,
    finished_at_local: null,
  };
}

function routine(id: string, title = id): RoutineResponse {
  return { id, title } as RoutineResponse;
}

describe("rangeFromParam", () => {
  it("accepts the offered ranges and falls back to 30 days", () => {
    expect(rangeFromParam("7")).toBe(7);
    expect(rangeFromParam("90")).toBe(90);
    expect(rangeFromParam(null)).toBe(30);
    expect(rangeFromParam("5")).toBe(30);
  });
});

describe("dayBounds", () => {
  it("returns local midnights oldest first, ending at tomorrow", () => {
    expect(dayBounds(NOW, 3)).toEqual([
      new Date(2026, 10, 14).getTime() / 1000,
      YESTERDAY,
      TODAY,
      new Date(2026, 10, 17).getTime() / 1000,
    ]);
  });

  it("crosses month boundaries", () => {
    expect(dayBounds(new Date(2026, 2, 1, 9).getTime() / 1000, 2)[0]).toBe(new Date(2026, 1, 28).getTime() / 1000);
  });
});

describe("buildUptime", () => {
  const routines = [routine("a", "Alpha"), routine("b", "Beta"), routine("c", "Gamma")];
  const runs = [
    run("a", TODAY + 100, "failed"),
    run("a", TODAY + 50),
    run("a", YESTERDAY + 10, "failed"),
    run("b", TODAY + 200),
    run("b", YESTERDAY + 20, "running"),
    run("b", YESTERDAY + 30, "unknown"),
    run("gone", TODAY + 300, "failed", "Deleted"),
    run("a", TODAY - 3 * 86_400, "failed"),
  ];
  const report = buildUptime(runs, routines, NOW, 2, 1000);
  const row = (id: string) => report.rows.find((r) => r.routineId === id)!;

  it("buckets runs by day and derives each day's state", () => {
    expect(row("a").days.map((d) => d.state)).toEqual(["down", "partial"]);
    expect(row("a").success).toBe(1);
    expect(row("a").failed).toBe(2);
    expect(row("a").uptime).toBeCloseTo(1 / 3);
    expect(row("b").days.map((d) => d.state)).toEqual(["none", "ok"]);
    expect(row("b").days[0]!.running).toBe(true);
    expect(row("c").days.every((d) => d.state === "none")).toBe(true);
    expect(row("c").uptime).toBeNull();
  });

  it("keeps a deleted routine's runs under the title they ran with", () => {
    expect(row("gone").title).toBe("Deleted");
  });

  it("orders lowest uptime first and routines without finished runs last", () => {
    expect(report.rows.map((r) => r.title)).toEqual(["Deleted", "Alpha", "Beta", "Gamma"]);
  });

  it("breaks uptime ties by title", () => {
    const tied = buildUptime([], [routine("z"), routine("y")], NOW, 2, 1000);
    expect(tied.rows.map((r) => r.title)).toEqual(["y", "z"]);
  });

  it("folds every routine into the fleet row and counts the KPIs", () => {
    expect(report.fleet.days.map((d) => [d.success, d.failed])).toEqual([
      [0, 1],
      [2, 2],
    ]);
    expect(report.fleet.uptime).toBeCloseTo(0.4);
    expect(report.daysWithFailures).toBe(2);
    expect(report.perfect).toBe(1);
    expect(report.partialSample).toBe(false);
  });

  it("ignores runs that start after today", () => {
    const later = buildUptime([run("a", TODAY + 86_400 + 5)], [], NOW, 2, 1000);
    expect(later.rows).toEqual([]);
    expect(later.fleet.uptime).toBeNull();
  });

  it("flags a capped sample that stops short of the range start", () => {
    expect(buildUptime([run("a", TODAY)], [], NOW, 2, 1).partialSample).toBe(true);
    expect(buildUptime([run("a", YESTERDAY - 1)], [], NOW, 2, 1).partialSample).toBe(false);
    expect(buildUptime([], [], NOW, 2, 1).partialSample).toBe(false);
  });
});

describe("uptimeLabel", () => {
  it("formats one decimal, a clean 100%, and a dash for no data", () => {
    expect(uptimeLabel(null)).toBe("—");
    expect(uptimeLabel(1)).toBe("100%");
    expect(uptimeLabel(0.9996)).toBe("99.9%");
    expect(uptimeLabel(2 / 3)).toBe("66.6%");
    expect(uptimeLabel(0)).toBe("0.0%");
  });
});

describe("dayLabel", () => {
  const day = (o: Partial<DayCell>): DayCell => ({
    from: TODAY,
    to: TODAY + 86_400,
    success: 0,
    failed: 0,
    running: false,
    state: "none",
    ...o,
  });

  it("describes empty, live, and finished days", () => {
    expect(dayLabel(day({}))).toMatch(/: no finished runs$/);
    expect(dayLabel(day({ running: true }))).toMatch(/: no finished runs \(running now\)$/);
    expect(dayLabel(day({ state: "partial", success: 2, failed: 1 }))).toMatch(/: 2 succeeded, 1 failed$/);
    expect(dayLabel(day({ state: "ok", success: 1, running: true }))).toMatch(/: 1 succeeded, 0 failed, 1\+ running$/);
  });
});

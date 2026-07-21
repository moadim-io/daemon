// Ported 1:1 from ui/src/routines/sparkline_tests.rs (removed).
import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { RUN_HISTORY_FETCH_LIMIT, SPARKLINE_LEN, groupRecentRuns, sparkTickClass } from "./sparkline";

function run(routineId: string, startedAt: number, status: FleetRunSummary["status"]): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: routineId,
    workbench: `${routineId}-${startedAt}`,
    started_at: startedAt,
    started_at_local: new Date(startedAt * 1000).toISOString(),
    finished_at: startedAt + 1,
    status,
    exit_code: null,
  };
}

describe("sparkline", () => {
  it("RUN_HISTORY_FETCH_LIMIT is 300", () => {
    expect(RUN_HISTORY_FETCH_LIMIT).toBe(300);
  });

  it("group_recent_runs empty input is empty map", () => {
    expect(groupRecentRuns([]).size).toBe(0);
  });

  it("group_recent_runs buckets by routine id", () => {
    const runs = [run("a", 300, "success"), run("b", 200, "failed"), run("a", 100, "failed")];
    const byRoutine = groupRecentRuns(runs);
    expect(byRoutine.size).toBe(2);
    expect(byRoutine.get("a")?.length).toBe(2);
    expect(byRoutine.get("b")?.length).toBe(1);
  });

  it("group_recent_runs reverses newest-first input to oldest-first", () => {
    const runs = [run("a", 300, "success"), run("a", 100, "failed")];
    const a = groupRecentRuns(runs).get("a") ?? [];
    expect(a[0]?.started_at).toBe(100);
    expect(a[1]?.started_at).toBe(300);
  });

  it("group_recent_runs caps at sparkline len per routine", () => {
    const runs = Array.from({ length: SPARKLINE_LEN + 5 }, (_, i) => run("a", i, "success"));
    expect(groupRecentRuns(runs).get("a")?.length).toBe(SPARKLINE_LEN);
  });

  it("group_recent_runs keeps the newest runs when capping", () => {
    // Input is newest-first; capping must keep the front (newest) slice, not the tail.
    const runs = Array.from({ length: SPARKLINE_LEN + 3 }, (_, i) => i)
      .reverse()
      .map((i) => run("a", i, "success"));
    const a = groupRecentRuns(runs).get("a") ?? [];
    expect(a.length).toBe(SPARKLINE_LEN);
    // Oldest-first after grouping, so the newest run is last.
    expect(a.at(-1)?.started_at).toBe(SPARKLINE_LEN + 2);
    expect(a[0]?.started_at).toBe(3);
  });

  it("spark_tick_class covers every variant", () => {
    expect(sparkTickClass("running")).toBe("spark-tick running");
    expect(sparkTickClass("success")).toBe("spark-tick success");
    expect(sparkTickClass("failed")).toBe("spark-tick failed");
    expect(sparkTickClass("unknown")).toBe("spark-tick unknown");
  });
});

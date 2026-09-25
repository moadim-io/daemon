import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { buildForecast, DEFAULT_ESTIMATE_SECS, medianDurations, overCapWindows, projectedFires } from "./forecastMath";

export function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "a",
    schedule: "0 * * * *",
    title: "Alpha",
    agent: "a",
    enabled: true,
    source: "",
    created_at: 0,
    updated_at: 0,
    agent_registered: false,
    agent_command_available: false,
    agent_setup_available: false,
    is_running: false,
    file_path: "",
    folder: null,
    slug: "routine",
    rel_path: "routine",
    flag_count: 0,
    env_keys: [],
    machines: ["m1"],
    ...overrides,
  };
}

export function run(o: Partial<FleetRunSummary>): FleetRunSummary {
  return {
    routine_id: "a",
    routine_title: "Alpha",
    workbench: "wb",
    started_at: 0,
    started_at_local: "",
    finished_at: 600,
    finished_at_local: "",
    status: "success",
    exit_code: 0,
    ...o,
  };
}

// 2026-06-21 12:00 local, on the hour.
const NOW = Math.floor(new Date(2026, 5, 21, 12, 0, 0).getTime() / 1000);
const H = 3_600;

describe("medianDurations", () => {
  it("takes the lower median of finished runs per routine, ignoring running/unfinished", () => {
    const m = medianDurations([
      run({ finished_at: 100 }),
      run({ finished_at: 300 }),
      run({ finished_at: 200 }),
      run({ finished_at: 900 }),
      run({ finished_at: null, status: "running" }),
      run({ routine_id: "b", started_at: 50, finished_at: 20 }),
    ]);
    expect(m.get("a")).toBe(200);
    expect(m.get("b")).toBe(0);
  });
});

describe("projectedFires", () => {
  it("lists fires inside the window, merged across schedules", () => {
    const fires = projectedFires(routine({ schedules: ["0 * * * *", "0 13 * * *"] }), NOW, NOW + 3 * H);
    expect(fires).toEqual([NOW + H, NOW + 2 * H]);
  });

  it("drops fires before snoozed_until and the next skip_runs fires", () => {
    expect(projectedFires(routine({ snoozed_until: NOW + 2 * H }), NOW, NOW + 4 * H)).toEqual([NOW + 2 * H, NOW + 3 * H]);
    expect(projectedFires(routine({ skip_runs: 2 }), NOW, NOW + 4 * H)).toEqual([NOW + 3 * H]);
  });

  it("is empty for an unparseable schedule", () => {
    expect(projectedFires(routine({ schedule: "nope" }), NOW, NOW + H)).toEqual([]);
  });
});

describe("buildForecast", () => {
  it("sizes bars by median, skips disabled/dormant routines, and flags over-cap windows", () => {
    const routines = [
      routine({ schedule: "0 13 * * *" }),
      routine({ id: "b", title: "Beta", schedule: "30 13 * * *" }),
      routine({ id: "off", title: "Off", enabled: false }),
      routine({ id: "dormant", title: "Dormant", machines: [] }),
    ];
    const f = buildForecast(routines, [run({ finished_at: 2 * H })], NOW, 1);
    expect(f.lanes.map((l) => l.title)).toEqual(["Alpha", "Beta"]);
    const [alpha, beta] = f.lanes;
    expect(alpha?.estimated).toBe(false);
    expect(alpha?.bars).toEqual([{ start: NOW + H, end: NOW + 3 * H, running: false }]);
    expect(beta?.estimated).toBe(true);
    expect(beta?.estimateSecs).toBe(DEFAULT_ESTIMATE_SECS);
    expect(f.peak).toBe(2);
    expect(f.overCap).toEqual([
      { from: NOW + 1.5 * H, to: NOW + 1.5 * H + DEFAULT_ESTIMATE_SECS, peak: 2, titles: ["Alpha", "Beta"] },
    ]);
    expect(buildForecast(routines, [], NOW, 0).overCap).toEqual([]);
  });

  it("counts a currently running run until its expected finish and floors tiny medians", () => {
    const f = buildForecast(
      [routine({ schedule: "0 0 1 1 *" })],
      [run({ finished_at: 1 }), run({ started_at: NOW - 30, finished_at: null, status: "running" })],
      NOW,
      1,
    );
    expect(f.lanes[0]?.estimateSecs).toBe(60);
    expect(f.lanes[0]?.bars).toEqual([{ start: NOW, end: NOW + 60, running: true }]);
  });

  it("is empty when nothing fires", () => {
    const f = buildForecast([routine({ schedule: "0 0 1 1 *" })], [], NOW, 2);
    expect(f.lanes).toEqual([]);
    expect(f.peak).toBe(0);
  });
});

describe("overCapWindows", () => {
  it("merges adjacent over-cap spans and keeps the peak", () => {
    const spans = [
      { from: 0, to: 10, count: 3 },
      { from: 10, to: 20, count: 4 },
      { from: 20, to: 30, count: 1 },
      { from: 30, to: 40, count: 3 },
    ];
    const w = overCapWindows(spans, [], 2);
    expect(w.map(({ from, to, peak }) => [from, to, peak])).toEqual([
      [0, 20, 4],
      [30, 40, 3],
    ]);
  });
});

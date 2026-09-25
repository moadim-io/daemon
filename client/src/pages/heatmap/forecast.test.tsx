import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { CapacityForecast } from "./CapacityForecast";
import { buildForecast, DEFAULT_ESTIMATE_SECS, medianDurations, overCapWindows, projectedFires } from "./forecastMath";

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
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

function run(o: Partial<FleetRunSummary>): FleetRunSummary {
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

const ROUTINES = [
  routine({ schedule: "0 13 * * *" }),
  routine({ id: "b", title: "Beta", schedule: "30 13 * * *" }),
];
const RUNS = [
  run({ finished_at: 7_200 }),
  run({ routine_id: "b", started_at: NOW - 60, finished_at: null, status: "running" }),
];

function renderForecast(cap: number | undefined) {
  return render(<CapacityForecast forecast={buildForecast(ROUTINES, RUNS, NOW, cap ?? 0)} cap={cap} />);
}

describe("CapacityForecast", () => {
  it("renders nothing when nothing is projected", () => {
    const { container } = render(<CapacityForecast forecast={buildForecast([], [], NOW, 1)} cap={1} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("draws lanes with median/default estimates and lists over-cap windows", () => {
    renderForecast(1);
    expect(screen.getByText(/projected peak 2 \/ cap 1 — 1 over-cap window$/)).toHaveClass("c-amber");
    expect(screen.getByRole("img", { name: /Alpha: fires .*, ~2h 0m median/ })).toHaveClass("runs-gantt-bar");
    expect(screen.getByRole("img", { name: /Beta: running now, ~10m \(default, no history\)/ })).toHaveClass("running");
    expect(screen.getByRole("img", { name: /Beta: fires .*default/ })).toHaveClass("est");
    expect(screen.getByRole("list", { name: "Over-cap windows" })).toHaveTextContent("13:30–13:40 · 2 projected Alpha, Beta");
    expect(document.querySelector(".runs-gantt-cap")).not.toBeNull();
    expect(document.querySelector(".runs-gantt-level.over")).not.toBeNull();
  });

  it("labels an unbounded and a loading cap without warnings", () => {
    const { unmount } = renderForecast(0);
    expect(screen.getByText(/projected peak 2 \(no cap\)$/)).not.toHaveClass("c-amber");
    expect(screen.queryByRole("list")).toBeNull();
    unmount();
    renderForecast(undefined);
    expect(screen.getByText(/^projected peak 2$/)).toBeInTheDocument();
    expect(document.querySelector(".runs-gantt-cap")).toBeNull();
  });

  it("pluralizes multiple over-cap windows", () => {
    const routines = [...ROUTINES, routine({ id: "c", title: "Gamma", schedule: "30 13,20 * * *" })];
    const runs = [run({ finished_at: 7_200 }), run({ routine_id: "b", finished_at: 36_000 })];
    render(<CapacityForecast forecast={buildForecast(routines, runs, NOW, 1)} cap={1} />);
    expect(screen.getByText(/over-cap windows$/)).toBeInTheDocument();
  });
});

import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { buildGantt, concurrencySpans, pct, runEnd } from "./ganttMath";

function run(o: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    routine_id: "a",
    routine_title: "Alpha",
    workbench: `wb-${Math.random()}`,
    started_at: 100,
    started_at_local: "",
    finished_at: 200,
    finished_at_local: "",
    status: "success",
    exit_code: 0,
    ...o,
  };
}

describe("runEnd", () => {
  it("uses finish, now for running, start for unknown", () => {
    expect(runEnd(run(), 1000)).toBe(200);
    expect(runEnd(run({ finished_at: 50 }), 1000)).toBe(100);
    expect(runEnd(run({ finished_at: null, status: "running" }), 1000)).toBe(1000);
    expect(runEnd(run({ finished_at: null, status: "unknown" }), 1000)).toBe(100);
  });
});

describe("concurrencySpans", () => {
  it("counts overlap and treats back-to-back as non-overlapping", () => {
    const spans = concurrencySpans(
      [
        { start: 10, end: 30 },
        { start: 20, end: 40 },
        { start: 40, end: 50 },
        { start: 45, end: 45 },
      ],
      0,
      60,
    );
    expect(spans).toEqual([
      { from: 0, to: 10, count: 0 },
      { from: 10, to: 20, count: 1 },
      { from: 20, to: 30, count: 2 },
      { from: 30, to: 40, count: 1 },
      { from: 40, to: 50, count: 1 },
      { from: 50, to: 60, count: 0 },
    ]);
  });

  it("returns one empty span with no bars", () => {
    expect(concurrencySpans([], 0, 10)).toEqual([{ from: 0, to: 10, count: 0 }]);
  });
});

describe("buildGantt", () => {
  it("is empty with no runs or an empty window", () => {
    expect(buildGantt([], 1000).lanes).toEqual([]);
    expect(buildGantt([run()], 1000, 1000).lanes).toEqual([]);
  });

  it("groups lanes by routine sorted by title, clips to window, drops out-of-window runs", () => {
    const g = buildGantt(
      [
        run({ routine_id: "z", routine_title: "Zeta", started_at: 150, finished_at: 400 }),
        run({ started_at: 100, finished_at: 300 }),
        run({ started_at: 10, finished_at: 20 }),
        run({ started_at: 2000, finished_at: 2100 }),
        run({ started_at: 900, finished_at: null, status: "running" }),
      ],
      1000,
      120,
    );
    expect(g.lanes.map((l) => l.title)).toEqual(["Alpha", "Zeta"]);
    expect(g.lanes[0]!.bars.map((b) => [b.start, b.end])).toEqual([
      [120, 300],
      [900, 1000],
    ]);
    expect(g.peak).toBe(2);
    expect(g.from).toBe(120);
    expect(g.to).toBe(1000);
  });

  it("defaults the window start to the oldest run", () => {
    expect(buildGantt([run({ started_at: 50, finished_at: 60 })], 100).from).toBe(50);
  });
});

describe("pct", () => {
  it("maps into the window", () => {
    expect(pct(150, 100, 200)).toBe(50);
  });
});

import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { DEFAULT_RUNS_FILTER, filterRuns, statusCounts } from "./runsFilter";

function run(overrides: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    routine_id: "r1",
    routine_title: "Nightly Audit",
    workbench: "nightly-audit-1000",
    started_at: 1_000,
    started_at_local: "",
    finished_at: 1_060,
    finished_at_local: "",
    status: "success",
    exit_code: 0,
    ...overrides,
  };
}

describe("filterRuns", () => {
  it("passes everything through the default filter", () => {
    const runs = [run(), run({ status: "failed" })];
    expect(filterRuns(runs, DEFAULT_RUNS_FILTER, 2_000)).toHaveLength(2);
  });

  it("filters by status facet", () => {
    const runs = [run({ status: "success" }), run({ status: "failed" }), run({ status: "running" })];
    const shown = filterRuns(runs, { ...DEFAULT_RUNS_FILTER, status: "failed" }, 2_000);
    expect(shown).toHaveLength(1);
    expect(shown[0]!.status).toBe("failed");
  });

  it("filters by case-insensitive routine title search", () => {
    const runs = [run({ routine_title: "Nightly Audit" }), run({ routine_title: "Weekly Digest" })];
    const shown = filterRuns(runs, { ...DEFAULT_RUNS_FILTER, query: "night" }, 2_000);
    expect(shown).toHaveLength(1);
    expect(shown[0]!.routine_title).toBe("Nightly Audit");
  });

  it("filters by recency window relative to now", () => {
    const now = 100_000;
    const runs = [
      run({ started_at: now - 30 }), // within the last hour
      run({ started_at: now - 3_601 }), // just outside the last hour
    ];
    const shown = filterRuns(runs, { ...DEFAULT_RUNS_FILTER, time: "1h" }, now);
    expect(shown).toHaveLength(1);
    expect(shown[0]!.started_at).toBe(now - 30);
  });

  it("combines status, query, and time facets", () => {
    const now = 100_000;
    const runs = [
      run({ routine_title: "Nightly Audit", status: "failed", started_at: now - 10 }),
      run({ routine_title: "Nightly Audit", status: "success", started_at: now - 10 }),
      run({ routine_title: "Weekly Digest", status: "failed", started_at: now - 10 }),
      run({ routine_title: "Nightly Audit", status: "failed", started_at: now - 999_999 }),
    ];
    const shown = filterRuns(runs, { query: "night", status: "failed", time: "24h" }, now);
    expect(shown).toHaveLength(1);
    expect(shown[0]).toBe(runs[0]);
  });
});

describe("statusCounts", () => {
  it("tallies runs by status", () => {
    const runs = [run({ status: "success" }), run({ status: "success" }), run({ status: "failed" }), run({ status: "running" })];
    expect(statusCounts(runs)).toEqual({ running: 1, success: 2, failed: 1, unknown: 0 });
  });

  it("returns all-zero counts for an empty list", () => {
    expect(statusCounts([])).toEqual({ running: 0, success: 0, failed: 0, unknown: 0 });
  });
});

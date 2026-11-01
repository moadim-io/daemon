import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { DEFAULT_RUNS_FILTER, filterRuns, runsToCsv, statusCounts, viewFromParams, viewToParams } from "./runsFilter";

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

describe("viewFromParams / viewToParams", () => {
  it("round-trips a full view", () => {
    const filter = { query: "audit", status: "failed" as const, time: "24h" as const };
    const range = { from: 100, to: 200 };
    const params = viewToParams(filter, range);
    expect(params.toString()).toBe("q=audit&status=failed&when=24h&from=100&to=200");
    expect(viewFromParams(params)).toEqual({ filter, range });
  });

  it("omits defaults so the bare URL stays clean", () => {
    expect(viewToParams(DEFAULT_RUNS_FILTER, null).toString()).toBe("");
    expect(viewFromParams(new URLSearchParams())).toEqual({ filter: DEFAULT_RUNS_FILTER, range: null });
  });

  it("falls back to defaults for unknown facets and malformed ranges", () => {
    expect(viewFromParams(new URLSearchParams("status=bogus&when=1y&from=x&to=5"))).toEqual({
      filter: DEFAULT_RUNS_FILTER,
      range: null,
    });
    expect(viewFromParams(new URLSearchParams("from=10&to=5")).range).toBeNull();
    expect(viewFromParams(new URLSearchParams("from=10")).range).toBeNull();
  });
});

const HEADER = "routine,routine_id,workbench,status,exit_code,started_at,finished_at,duration_secs";

describe("runsToCsv", () => {
  it("writes a header plus one ISO-timestamped row per run", () => {
    expect(runsToCsv([run()])).toBe(
      HEADER + "\r\n" +
        "Nightly Audit,r1,nightly-audit-1000,success,0,1970-01-01T00:16:40.000Z,1970-01-01T00:17:40.000Z,60\r\n",
    );
  });

  it("leaves running-run fields blank and quotes cells with commas or quotes", () => {
    const csv = runsToCsv([
      run({ routine_title: 'Deploy "prod", eu', status: "running", finished_at: null, exit_code: null }),
    ]);
    expect(csv.split("\r\n")[1]).toBe('"Deploy ""prod"", eu",r1,nightly-audit-1000,running,,1970-01-01T00:16:40.000Z,,');
  });

  it("emits only the header for no runs", () => {
    expect(runsToCsv([])).toBe(HEADER + "\r\n");
  });
});

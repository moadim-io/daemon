import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { buildHistogram, pickStep, runsInRange, TARGET_BUCKETS } from "./histogramMath";

function run(started_at: number, status: FleetRunSummary["status"] = "success"): FleetRunSummary {
  return {
    routine_id: "r1",
    routine_title: "Nightly Audit",
    workbench: `wb-${started_at}-${status}`,
    started_at,
    started_at_local: "",
    finished_at: started_at + 60,
    finished_at_local: "",
    status,
    exit_code: 0,
  };
}

describe("pickStep", () => {
  it("picks the smallest round step covering the span in the target bar count", () => {
    expect(pickStep(0)).toBe(60);
    expect(pickStep(3_600)).toBe(300);
    expect(pickStep(86_400)).toBe(3_600);
    expect(pickStep(7 * 86_400)).toBe(12 * 3_600);
  });

  it("falls back to the widest step for very long spans", () => {
    expect(pickStep(365 * 86_400)).toBe(7 * 86_400);
  });
});

describe("buildHistogram", () => {
  it("is empty with no runs and no explicit start", () => {
    expect(buildHistogram([], 10_000)).toEqual({ buckets: [], step: 0, max: 0 });
  });

  it("is empty when the start is in the future", () => {
    expect(buildHistogram([], 10_000, 20_000).buckets).toHaveLength(0);
  });

  it("stacks runs per aligned bucket by status and tracks the max", () => {
    const now = 86_400;
    const h = buildHistogram([run(0), run(10, "failed"), run(3_700), run(3_800, "running")], now);
    expect(h.step).toBe(3_600);
    expect(h.buckets).toHaveLength(25);
    expect(h.buckets.length).toBeGreaterThanOrEqual(TARGET_BUCKETS);
    expect(h.buckets[0]).toMatchObject({ from: 0, to: 3_600, total: 2, counts: { success: 1, failed: 1 } });
    expect(h.buckets[1]!.counts.running).toBe(1);
    expect(h.buckets[2]!.total).toBe(0);
    expect(h.max).toBe(2);
  });

  it("spans an explicit window and drops runs outside it", () => {
    const h = buildHistogram([run(100), run(5_000, "unknown")], 7_200, 3_600);
    expect(h.buckets[0]!.from).toBe(3_600);
    expect(h.buckets.reduce((n, b) => n + b.total, 0)).toBe(1);
    expect(h.buckets.some((b) => b.counts.unknown === 1)).toBe(true);
  });
});

describe("runsInRange", () => {
  it("keeps runs starting inside the half-open range", () => {
    const runs = [run(99), run(100), run(199), run(200)];
    expect(runsInRange(runs, { from: 100, to: 200 }).map((r) => r.started_at)).toEqual([100, 199]);
  });
});

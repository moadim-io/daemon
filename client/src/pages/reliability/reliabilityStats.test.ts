import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import {
  SAMPLE_LEN,
  computeReliability,
  fleetSummary,
  isFlaky,
  rateClass,
  rateLabel,
  streakClass,
  streakLabel,
  successRate,
  type RoutineReliability,
} from "./reliabilityStats";

function run(
  routineId: string,
  startedAt: number,
  status: FleetRunSummary["status"],
  durationSecs?: number,
): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: `Routine ${routineId}`,
    workbench: `${routineId}-${startedAt}`,
    started_at: startedAt,
    started_at_local: new Date(startedAt * 1000).toISOString(),
    finished_at: durationSecs === undefined ? null : startedAt + durationSecs,
    status,
    exit_code: status === "success" ? 0 : status === "failed" ? 1 : null,
  };
}

/** `n` newest-first runs for one routine, alternating/failing per `statuses`, each `durationSecs` long. */
function series(
  routineId: string,
  statuses: FleetRunSummary["status"][],
  durationSecs = 10,
): FleetRunSummary[] {
  // newest-first: first element has the highest started_at.
  return statuses.map((status, i) => run(routineId, 1_000 - i * 60, status, durationSecs));
}

describe("computeReliability", () => {
  it("empty input yields no items", () => {
    expect(computeReliability([])).toEqual([]);
  });

  it("ignores running/unknown runs when bucketing", () => {
    const runs = [run("a", 100, "running"), run("a", 90, "unknown")];
    expect(computeReliability(runs)).toEqual([]);
  });

  it("computes success rate and streak for an all-success routine", () => {
    const runs = series("a", ["success", "success", "success"]);
    const [item] = computeReliability(runs);
    expect(item?.sampleSize).toBe(3);
    expect(item?.successes).toBe(3);
    expect(item?.streak).toEqual({ kind: "success", count: 3 });
    expect(successRate(item as RoutineReliability)).toBe(1);
  });

  it("computes an active failure streak from the newest run backward", () => {
    // newest-first: failed, failed, success -> active failure streak of 2.
    const runs = series("a", ["failed", "failed", "success"]);
    const [item] = computeReliability(runs);
    expect(item?.streak).toEqual({ kind: "failure", count: 2 });
  });

  it("caps the sample at SAMPLE_LEN, keeping the newest runs", () => {
    const statuses: FleetRunSummary["status"][] = Array.from({ length: SAMPLE_LEN + 5 }, () => "success");
    const runs = series("a", statuses);
    const [item] = computeReliability(runs);
    expect(item?.sampleSize).toBe(SAMPLE_LEN);
  });

  it("flags a routine as flaky once its flip rate crosses the threshold", () => {
    const runs = series("a", ["success", "failed", "success", "failed", "success"]);
    const [item] = computeReliability(runs);
    expect(item?.flips).toBe(4);
    expect(isFlaky(item as RoutineReliability)).toBe(true);
  });

  it("does not flag flaky below the minimum sample size", () => {
    const runs = series("a", ["success", "failed"]);
    const [item] = computeReliability(runs);
    expect(isFlaky(item as RoutineReliability)).toBe(false);
  });

  it("ranks worst-first: active failure streak outranks a merely low success rate", () => {
    const runs = [
      ...series("steady-fail", ["failed", "failed", "failed"]),
      ...series("recovered", ["success", "failed", "failed"]),
    ];
    const items = computeReliability(runs);
    expect(items[0]?.routineId).toBe("steady-fail");
  });

  it("computes p50/p95 duration and flags a slower-trend regression", () => {
    // Newest-first: 8 runs. Newer half (newest 4) at 100s, older half (oldest 4) at 20s.
    const runs = [
      ...series("a", ["success", "success", "success", "success"], 100),
      ...Array.from({ length: 4 }, (_, i) => run("a", 600 - i * 60, "success", 20)),
    ];
    const [item] = computeReliability(runs);
    expect(item?.durationsSecs.length).toBe(8);
    expect(item?.p50Secs).not.toBeNull();
    expect(item?.p95Secs).not.toBeNull();
    expect(item?.regressing).toBe(true);
  });

  it("does not flag a regression when durations are stable", () => {
    const runs = series("a", Array.from({ length: 10 }, () => "success" as const), 30);
    const [item] = computeReliability(runs);
    expect(item?.regressing).toBe(false);
  });

  it("does not flag a regression below the minimum timed sample", () => {
    const runs = [run("a", 100, "success", 5), run("a", 40, "success", 500)];
    const [item] = computeReliability(runs);
    expect(item?.regressing).toBe(false);
  });

  it("omits runs missing a finished_at from the duration sample", () => {
    const runs = [run("a", 100, "success", 10), run("a", 40, "success")];
    const [item] = computeReliability(runs);
    expect(item?.sampleSize).toBe(2);
    expect(item?.durationsSecs.length).toBe(1);
  });
});

describe("fleetSummary", () => {
  it("aggregates an empty item list to zeros/nulls", () => {
    const summary = fleetSummary([]);
    expect(summary).toEqual({
      sampleSize: 0,
      successes: 0,
      failingCount: 0,
      flakyCount: 0,
      p50Secs: null,
      p95Secs: null,
      regressingCount: 0,
    });
  });

  it("sums per-routine counts and aggregates durations fleet-wide", () => {
    const runs = [
      ...series("a", ["success", "success"], 10),
      ...series("b", ["failed", "failed"], 50),
    ];
    const items = computeReliability(runs);
    const summary = fleetSummary(items);
    expect(summary.sampleSize).toBe(4);
    expect(summary.successes).toBe(2);
    expect(summary.failingCount).toBe(1);
    expect(summary.p50Secs).not.toBeNull();
  });
});

describe("badge helpers", () => {
  it("streakClass/streakLabel cover all streak kinds", () => {
    expect(streakClass({ kind: "success", count: 1 })).toBe("run-status success");
    expect(streakClass({ kind: "failure", count: 1 })).toBe("run-status failed");
    expect(streakClass({ kind: "none" })).toBe("run-status unknown");
    expect(streakLabel({ kind: "success", count: 3 })).toBe("3 OK");
    expect(streakLabel({ kind: "failure", count: 2 })).toBe("2 FAILING");
    expect(streakLabel({ kind: "none" })).toBe("—");
  });

  it("rateClass/rateLabel bucket by success rate", () => {
    expect(rateClass(null)).toBe("run-status unknown");
    expect(rateClass(0.95)).toBe("run-status success");
    expect(rateClass(0.75)).toBe("run-status running");
    expect(rateClass(0.5)).toBe("run-status failed");
    expect(rateLabel(null)).toBe("—");
    expect(rateLabel(0.876)).toBe("88%");
  });
});

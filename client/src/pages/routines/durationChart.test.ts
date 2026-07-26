import { describe, expect, it } from "vitest";
import type { RunSummary } from "../../api/hooks";
import { MIN_BAR_PCT, buildDurationBars } from "./durationChart";

function run(startedAt: number, finishedAt: number | undefined, status: RunSummary["status"]): RunSummary {
  return {
    workbench: `wb-${startedAt}`,
    started_at: startedAt,
    started_at_local: new Date(startedAt * 1000).toISOString(),
    finished_at: finishedAt,
    status,
    exit_code: status === "failed" ? 1 : status === "success" ? 0 : null,
  };
}

describe("buildDurationBars", () => {
  it("empty input yields empty output", () => {
    expect(buildDurationBars([])).toEqual([]);
  });

  it("scales bar width relative to the slowest finished run", () => {
    const bars = buildDurationBars([run(0, 100, "success"), run(200, 250, "success")]);
    expect(bars[0]?.durationSecs).toBe(100);
    expect(bars[0]?.widthPct).toBe(100);
    expect(bars[1]?.durationSecs).toBe(50);
    expect(bars[1]?.widthPct).toBe(50);
  });

  it("in-flight runs (no finished_at) get the floor width and null duration", () => {
    const bars = buildDurationBars([run(0, undefined, "running")]);
    expect(bars[0]?.durationSecs).toBeNull();
    expect(bars[0]?.widthPct).toBe(MIN_BAR_PCT);
  });

  it("floors very short finished runs at MIN_BAR_PCT so they stay clickable", () => {
    const bars = buildDurationBars([run(0, 1, "success"), run(0, 1000, "success")]);
    expect(bars[0]?.widthPct).toBe(MIN_BAR_PCT);
  });

  it("preserves input order and carries exit code/status through", () => {
    const bars = buildDurationBars([run(10, 20, "failed"), run(30, 40, "success")]);
    expect(bars.map((b) => b.workbench)).toEqual(["wb-10", "wb-30"]);
    expect(bars[0]?.status).toBe("failed");
    expect(bars[0]?.exitCode).toBe(1);
  });

  it("a single unfinished run doesn't divide by zero", () => {
    const bars = buildDurationBars([run(0, undefined, "running")]);
    expect(Number.isFinite(bars[0]?.widthPct)).toBe(true);
  });
});

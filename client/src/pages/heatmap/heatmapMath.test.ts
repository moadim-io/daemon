import { describe, expect, it } from "vitest";
import {
  computeHeatmap,
  dayLabel,
  dayTotals,
  filledCells,
  heatFilterAccepts,
  heatFilterLabel,
  HEAT_DAYS,
  HEAT_HOURS,
  hourTotals,
  intensityLevel,
  peakLabel,
  sourcesOf,
  type Heatmap,
  type HeatSource,
} from "./heatmapMath";
import type { RoutineResponse } from "../../api/hooks";

/** A fixed reference instant: Mon 2026-06-22 10:00:00 local. Off midnight and
 * noon so per-day schedules land unambiguously inside the window. Ported 1:1
 * from `ui/src/schedule_heatmap_tests.rs`. */
const now = () => new Date(2026, 5, 22, 10, 0, 0);
const today = () => new Date(2026, 5, 22);

function source(schedule: string, enabled: boolean): HeatSource {
  return { kind: "routine", schedule, enabled };
}

// ─── HeatFilter ─────────────────────────────────────────────────────────────

describe("HeatFilter", () => {
  it("accepts by kind", () => {
    expect(heatFilterAccepts("all", "routine")).toBe(true);
    expect(heatFilterAccepts("routine", "routine")).toBe(true);
  });

  it("has labels", () => {
    expect(heatFilterLabel("all")).toBe("ALL");
    expect(heatFilterLabel("routine")).toBe("ROUTINES");
  });
});

// ─── computeHeatmap ─────────────────────────────────────────────────────────

describe("computeHeatmap", () => {
  it("fills one cell per day for a daily noon schedule", () => {
    // From 10:00 today, "every day at 12:00" fires once on each of the 7 days
    // in the window (today's noon is still ahead), all in the hour-12 column.
    const map = computeHeatmap([source("0 12 * * *", true)], now(), "all");

    expect(map.total).toBe(7);
    expect(map.maxCell).toBe(1);
    expect(map.peak).toEqual([0, 12]);
    for (let day = 0; day < HEAT_DAYS; day++) {
      expect(map.grid[day]?.[12]).toBe(1);
      expect(map.grid[day]?.[0]).toBe(0);
    }
  });

  it("leaves elapsed hours today empty", () => {
    // "Every day at 08:00" — 08:00 today already passed at 10:00, so today's
    // row is empty while the other six days each get one fire.
    const map = computeHeatmap([source("0 8 * * *", true)], now(), "all");

    expect(map.grid[0]?.[8]).toBe(0);
    expect(map.total).toBe(6);
    expect(map.peak).toEqual([1, 8]);
  });

  it("ignores disabled sources", () => {
    const map = computeHeatmap([source("0 12 * * *", false)], now(), "all");
    expect(map.total).toBe(0);
    expect(map.peak).toBeUndefined();
  });

  it("counts zero for a far-future schedule outside the window", () => {
    // 1 January fires well beyond the 7-day window from late June.
    const map = computeHeatmap([source("0 0 1 1 *", true)], now(), "all");
    expect(map.total).toBe(0);
  });

  it("skips an invalid schedule", () => {
    const map = computeHeatmap(
      [source("not a cron", true), source("0 12 * * *", true)],
      now(),
      "all",
    );
    expect(map.total).toBe(7);
  });

  it("filter restricts counted sources", () => {
    const sources = [source("0 12 * * *", true), source("0 12 * * *", true)];
    expect(computeHeatmap(sources, now(), "all").total).toBe(14);
    expect(computeHeatmap(sources, now(), "routine").total).toBe(14);
  });

  it("stacks collisions in one cell and sets the peak", () => {
    // Two daily-noon schedules pile two fires into each noon cell.
    const sources = [source("0 12 * * *", true), source("0 12 * * *", true)];
    const map = computeHeatmap(sources, now(), "all");
    expect(map.maxCell).toBe(2);
    expect(map.grid[0]?.[12]).toBe(2);
    expect(map.peak).toEqual([0, 12]);
  });

  it("produces a zeroed grid for empty sources", () => {
    const map = computeHeatmap([], now(), "all");
    expect(map.grid.length).toBe(HEAT_DAYS);
    expect(map.grid[0]?.length).toBe(HEAT_HOURS);
    expect(map.total).toBe(0);
    expect(map.maxCell).toBe(0);
    expect(map.peak).toBeUndefined();
    expect(map.sources).toBe(0);
  });

  it("counts sources that fire within the window", () => {
    const active = source("0 12 * * *", true);
    const disabled = source("0 12 * * *", false);
    const map = computeHeatmap([active, disabled], now(), "all");
    expect(map.sources).toBe(1);
  });
});

// ─── intensityLevel ─────────────────────────────────────────────────────────

describe("intensityLevel", () => {
  it("buckets into five steps", () => {
    expect(intensityLevel(0, 4)).toBe(0);
    expect(intensityLevel(5, 0)).toBe(0); // guard: zero max never divides
    expect(intensityLevel(1, 4)).toBe(1);
    expect(intensityLevel(2, 4)).toBe(2);
    expect(intensityLevel(3, 4)).toBe(3);
    expect(intensityLevel(4, 4)).toBe(4);
    expect(intensityLevel(1, 100)).toBe(1); // tiny ratio still reaches step 1
    expect(intensityLevel(100, 100)).toBe(4);
  });
});

// ─── axis totals ────────────────────────────────────────────────────────────

describe("axis totals", () => {
  it("day and hour totals sum the grid", () => {
    const sources = [source("0 12 * * *", true), source("0 12 * * *", true)];
    const map = computeHeatmap(sources, now(), "all");

    const days = dayTotals(map);
    expect(days.length).toBe(HEAT_DAYS);
    expect(days.every((d) => d === 2)).toBe(true); // two noon fires each day

    const hours = hourTotals(map);
    expect(hours.length).toBe(HEAT_HOURS);
    expect(hours[12]).toBe(14); // every day's two noon fires land in hour 12
    expect(hours[0]).toBe(0);
  });
});

// ─── peakLabel / dayLabel ───────────────────────────────────────────────────

describe("peakLabel", () => {
  it("reads weekday, hour, and count", () => {
    const single = computeHeatmap([source("0 12 * * *", true)], now(), "all");
    expect(peakLabel(single, today())).toBe("MON 12:00 · 1 run");

    const double = computeHeatmap(
      [source("0 12 * * *", true), source("0 12 * * *", true)],
      now(),
      "all",
    );
    expect(peakLabel(double, today())).toBe("MON 12:00 · 2 runs");
  });

  it("is undefined for an empty grid", () => {
    const map = computeHeatmap([], now(), "all");
    expect(peakLabel(map, today())).toBeUndefined();
  });
});

describe("dayLabel", () => {
  it("counts weekdays forward from today", () => {
    expect(dayLabel(today(), 0)).toBe("MON 22");
    expect(dayLabel(today(), 1)).toBe("TUE 23");
    expect(dayLabel(today(), 6)).toBe("SUN 28");
  });
});

// ─── record → source mappers ────────────────────────────────────────────────

function routine(schedule: string, enabled: boolean): RoutineResponse {
  return {
    id: "rid",
    schedule,
    title: "t",
    agent: "a",
    enabled,
    source: "",
    created_at: 0,
    updated_at: 0,
    agent_registered: false,
    agent_command_available: false,
    agent_setup_available: false,
    is_running: false,
    file_path: "",
    flag_count: 0,
    env_keys: [],
  };
}

describe("sourcesOf", () => {
  it("maps records preserving kind and enabled", () => {
    const sources = sourcesOf([routine("0 0 * * *", false)]);
    expect(sources.length).toBe(1);
    expect(sources[0]?.kind).toBe("routine");
    expect(sources[0]?.schedule).toBe("0 0 * * *");
    expect(sources[0]?.enabled).toBe(false);
  });
});

describe("filledCells", () => {
  it("counts nonempty cells only", () => {
    const map: Heatmap = computeHeatmap([source("0 12 * * *", true)], now(), "all");
    // One non-empty cell (hour 12) on each of the 7 days.
    expect(filledCells(map)).toBe(HEAT_DAYS);
  });
});

import { describe, expect, it } from "vitest";
import {
  bucketDayFires,
  dayTimelineLabel,
  timelineItemsOf,
  type TimelineItem,
} from "./dayTimelineMath";
import type { RoutineResponse } from "../../api/hooks";

// `fireTimesOnDay`'s own behavior (midnight boundary, adjacent-day exclusion, invalid schedules)
// is covered by `lib/schedule.test.ts` — this file only covers the bucketing built on top of it.
const day = () => new Date(2026, 5, 21); // Sun 2026-06-21

describe("bucketDayFires", () => {
  it("buckets each item's fires by hour and sorts within the bucket", () => {
    const items: TimelineItem[] = [
      { label: "A", schedule: "30 9 * * *", snoozed: false, flagCount: 0 },
      { label: "B", schedule: "0 9 * * *", snoozed: true, flagCount: 2 },
    ];
    const buckets = bucketDayFires(items, day());
    expect(buckets.length).toBe(24);
    expect(buckets[9]?.map((e) => e.label)).toEqual(["B", "A"]); // 09:00 before 09:30
    expect(buckets[9]?.[0]?.snoozed).toBe(true);
    expect(buckets[9]?.[0]?.flagCount).toBe(2);
    expect(buckets.filter((b) => b.length > 0).length).toBe(1);
  });

  it("is all-empty buckets when nothing fires", () => {
    const buckets = bucketDayFires([], day());
    expect(buckets.every((b) => b.length === 0)).toBe(true);
  });
});

describe("dayTimelineLabel", () => {
  it("formats weekday, month, day, and year", () => {
    expect(dayTimelineLabel(day())).toBe("SUN · JUN 21 2026");
  });
});

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "rid",
    schedule: "0 12 * * *",
    title: "Nightly build",
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
    flag_count: 3,
    env_keys: [],
    ...overrides,
  };
}

describe("timelineItemsOf", () => {
  const now = new Date(2026, 5, 21, 12, 0, 0);

  it("drops disabled routines and maps the rest", () => {
    const items = timelineItemsOf([routine({ enabled: false }), routine()], now);
    expect(items.length).toBe(1);
    expect(items[0]).toEqual({
      label: "Nightly build",
      schedule: "0 12 * * *",
      snoozed: false,
      flagCount: 3,
    });
  });

  it("flags a routine snoozed until a future time", () => {
    const future = Math.floor(now.getTime() / 1000) + 3_600;
    const items = timelineItemsOf([routine({ snoozed_until: future })], now);
    expect(items[0]?.snoozed).toBe(true);
  });

  it("is not snoozed once snoozed_until has passed", () => {
    const past = Math.floor(now.getTime() / 1000) - 3_600;
    const items = timelineItemsOf([routine({ snoozed_until: past })], now);
    expect(items[0]?.snoozed).toBe(false);
  });

  it("counts a positive skip_runs as snoozed", () => {
    const items = timelineItemsOf([routine({ skip_runs: 2 })], now);
    expect(items[0]?.snoozed).toBe(true);
  });
});

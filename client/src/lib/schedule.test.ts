import { describe, expect, it } from "vitest";
import {
  firesWithin,
  fmtUntil,
  fmtWhen,
  monthStart,
  nextFireAfter,
  nextFires,
  occurrencesPerDay,
} from "./schedule";

/** Fixed reference instant: Sun 2026-06-21 12:00:30 local. Off the minute boundary. */
const now = () => new Date(2026, 5, 21, 12, 0, 30);

describe("nextFireAfter", () => {
  it("returns the next top-of-hour fire", () => {
    const then = nextFireAfter("0 * * * *", now());
    expect(then).toEqual(new Date(2026, 5, 21, 13, 0, 0));
  });

  it("rejects invalid/empty schedules", () => {
    expect(nextFireAfter("not a cron", now())).toBeUndefined();
    expect(nextFireAfter("", now())).toBeUndefined();
  });
});

describe("firesWithin", () => {
  it("is true when the next fire is inside the window", () => {
    expect(firesWithin("0 * * * *", now(), 60 * 60_000)).toBe(true);
  });

  it("is false when the next fire is beyond the window", () => {
    expect(firesWithin("0 * * * *", now(), 30 * 60_000)).toBe(false);
  });

  it("is false for an invalid schedule", () => {
    expect(firesWithin("nonsense", now(), 60 * 60_000)).toBe(false);
  });
});

describe("fmtUntil", () => {
  it("is 'now' when due", () => {
    expect(fmtUntil(now(), now())).toBe("now");
  });

  it("is sub-minute", () => {
    expect(fmtUntil(now(), new Date(now().getTime() + 30_000))).toBe("in <1m");
  });

  it("is minutes", () => {
    expect(fmtUntil(now(), new Date(now().getTime() + 5 * 60_000))).toBe("in 5m");
  });

  it("is whole hours", () => {
    expect(fmtUntil(now(), new Date(now().getTime() + 2 * 3_600_000))).toBe("in 2h");
  });

  it("is hours and minutes", () => {
    const then = new Date(now().getTime() + 2 * 3_600_000 + 10 * 60_000);
    expect(fmtUntil(now(), then)).toBe("in 2h 10m");
  });

  it("is days", () => {
    expect(fmtUntil(now(), new Date(now().getTime() + 3 * 86_400_000))).toBe("in 3d");
  });
});

describe("fmtWhen", () => {
  it("is a bare time for today", () => {
    expect(fmtWhen(now(), new Date(now().getTime() + 3_600_000))).toBe("13:00");
  });

  it("is 'tomorrow' prefixed for the next day", () => {
    expect(fmtWhen(now(), new Date(now().getTime() + 86_400_000))).toBe("tomorrow 12:00");
  });

  it("uses month and day further out", () => {
    expect(fmtWhen(now(), new Date(now().getTime() + 3 * 86_400_000))).toBe("Jun 24, 12:00");
  });
});

describe("monthStart", () => {
  it("same month", () => {
    expect(monthStart(new Date(2024, 5, 15), 0)).toEqual(new Date(2024, 5, 1));
  });

  it("next month across a year boundary", () => {
    expect(monthStart(new Date(2024, 11, 31), 1)).toEqual(new Date(2025, 0, 1));
  });

  it("previous month across a year boundary", () => {
    expect(monthStart(new Date(2024, 0, 10), -1)).toEqual(new Date(2023, 11, 1));
  });
});

describe("occurrencesPerDay", () => {
  it("returns undefined for an invalid schedule", () => {
    expect(occurrencesPerDay("not-a-cron", new Date(2024, 5, 1))).toBeUndefined();
  });

  it("fills exactly 1 fire per day for a daily schedule", () => {
    const counts = occurrencesPerDay("0 12 * * *", new Date(2024, 5, 1));
    expect(counts).toBeDefined();
    expect(counts!.every((c) => c === 1)).toBe(true);
  });
});

describe("nextFires", () => {
  it("returns n sequential fires", () => {
    const fires = nextFires("0 * * * *", now(), 3);
    expect(fires).toEqual([
      new Date(2026, 5, 21, 13, 0, 0),
      new Date(2026, 5, 21, 14, 0, 0),
      new Date(2026, 5, 21, 15, 0, 0),
    ]);
  });

  it("is empty for an invalid schedule", () => {
    expect(nextFires("not a cron", now(), 5)).toEqual([]);
    expect(nextFires("", now(), 5)).toEqual([]);
  });

  it("is empty for n = 0", () => {
    expect(nextFires("0 * * * *", now(), 0)).toEqual([]);
  });
});

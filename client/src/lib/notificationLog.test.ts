import { beforeEach, describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../api/hooks";
import {
  entriesForFailures,
  loadNotificationLog,
  markAllRead,
  MAX_ENTRIES,
  saveNotificationLog,
  unreadCount,
  type NotificationEntry,
} from "./notificationLog";

function run(overrides: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    routine_id: "r1",
    routine_title: "Nightly backup",
    workbench: "nightly-backup-1000",
    started_at: 1000,
    started_at_local: "",
    finished_at: 1010,
    finished_at_local: null,
    exit_code: 1,
    status: "failed",
    ...overrides,
  };
}

describe("loadNotificationLog / saveNotificationLog", () => {
  beforeEach(() => localStorage.clear());

  it("returns an empty log when nothing is persisted", () => {
    expect(loadNotificationLog()).toEqual([]);
  });

  it("round-trips a saved log", () => {
    const entries: NotificationEntry[] = [
      { id: "a", routineId: "r1", routineTitle: "A", message: "Run failed", atSecs: 1, read: false },
    ];
    saveNotificationLog(entries);
    expect(loadNotificationLog()).toEqual(entries);
  });

  it("ignores corrupt storage", () => {
    localStorage.setItem("moadim.notification-log", "{not json");
    expect(loadNotificationLog()).toEqual([]);
  });

  it("drops malformed entries but keeps well-formed ones", () => {
    localStorage.setItem(
      "moadim.notification-log",
      JSON.stringify([{ id: "a" }, { id: "b", routineId: "r1", routineTitle: "B", message: "m", atSecs: 1, read: true }]),
    );
    expect(loadNotificationLog()).toEqual([
      { id: "b", routineId: "r1", routineTitle: "B", message: "m", atSecs: 1, read: true },
    ]);
  });
});

describe("entriesForFailures", () => {
  it("builds an unread entry for a freshly-failed run", () => {
    const result = entriesForFailures([], [run()]);
    expect(result).toEqual([
      {
        id: "nightly-backup-1000",
        routineId: "r1",
        routineTitle: "Nightly backup",
        message: "Run failed (exit 1)",
        atSecs: 1010,
        read: false,
      },
    ]);
  });

  it("falls back to started_at when finished_at is unknown", () => {
    const result = entriesForFailures([], [run({ finished_at: null, exit_code: null })]);
    expect(result[0]?.atSecs).toBe(1000);
    expect(result[0]?.message).toBe("Run failed");
  });

  it("prepends new entries ahead of existing ones", () => {
    const existing: NotificationEntry[] = [
      { id: "old", routineId: "r2", routineTitle: "Old", message: "Run failed", atSecs: 1, read: true },
    ];
    const result = entriesForFailures(existing, [run()]);
    expect(result.map((e) => e.id)).toEqual(["nightly-backup-1000", "old"]);
  });

  it("dedupes against a run id already present", () => {
    const existing: NotificationEntry[] = [
      { id: "nightly-backup-1000", routineId: "r1", routineTitle: "Nightly backup", message: "Run failed", atSecs: 1010, read: true },
    ];
    const result = entriesForFailures(existing, [run()]);
    expect(result).toBe(existing);
  });

  it("returns the same reference when there are no failures", () => {
    const existing: NotificationEntry[] = [];
    expect(entriesForFailures(existing, [])).toBe(existing);
  });

  it("caps the log at MAX_ENTRIES, dropping the oldest", () => {
    const existing: NotificationEntry[] = Array.from({ length: MAX_ENTRIES }, (_, i) => ({
      id: `old-${i}`,
      routineId: "r2",
      routineTitle: "Old",
      message: "Run failed",
      atSecs: i,
      read: true,
    }));
    const result = entriesForFailures(existing, [run()]);
    expect(result).toHaveLength(MAX_ENTRIES);
    expect(result[0]?.id).toBe("nightly-backup-1000");
    expect(result.some((e) => e.id === `old-${MAX_ENTRIES - 1}`)).toBe(false);
  });
});

describe("unreadCount", () => {
  it("counts only unread entries", () => {
    const entries: NotificationEntry[] = [
      { id: "a", routineId: "r1", routineTitle: "A", message: "m", atSecs: 1, read: false },
      { id: "b", routineId: "r1", routineTitle: "B", message: "m", atSecs: 1, read: true },
      { id: "c", routineId: "r1", routineTitle: "C", message: "m", atSecs: 1, read: false },
    ];
    expect(unreadCount(entries)).toBe(2);
  });

  it("is zero for an empty log", () => {
    expect(unreadCount([])).toBe(0);
  });
});

describe("markAllRead", () => {
  it("marks every entry read", () => {
    const entries: NotificationEntry[] = [
      { id: "a", routineId: "r1", routineTitle: "A", message: "m", atSecs: 1, read: false },
      { id: "b", routineId: "r1", routineTitle: "B", message: "m", atSecs: 1, read: true },
    ];
    expect(markAllRead(entries).every((e) => e.read)).toBe(true);
  });

  it("returns the same reference when already all-read", () => {
    const entries: NotificationEntry[] = [
      { id: "a", routineId: "r1", routineTitle: "A", message: "m", atSecs: 1, read: true },
    ];
    expect(markAllRead(entries)).toBe(entries);
  });
});

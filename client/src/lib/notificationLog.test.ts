import { beforeEach, describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../api/hooks";
import {
  entriesForFailures,
  entriesForMissedScheduledRuns,
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

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "r1",
    schedule: "0 6 * * *",
    schedules: [],
    title: "Morning review",
    agent: "claude",
    model: null,
    goal: null,
    repositories: [],
    machines: ["box"],
    enabled: true,
    source: "managed",
    created_at: 1,
    updated_at: 1,
    last_manual_trigger_at: null,
    last_scheduled_trigger_at: 1000,
    snoozed_until: null,
    skip_runs: null,
    power_saving: false,
    power_saving_exempt: false,
    consecutive_failures: 0,
    auto_disabled_reason: null,
    ttl_secs: null,
    max_runtime_secs: null,
    failure_threshold: null,
    tags: [],
    agent_registered: true,
    agent_command_available: true,
    agent_setup_available: true,
    file_path: "/tmp/routine.toml",
    folder: null,
    slug: "morning-review",
    rel_path: "morning-review",
    schedule_description: "At 06:00",
    schedule_descriptions: ["At 06:00"],
    timezone: "UTC",
    flag_count: 0,
    next_run_at: 2000,
    missed_scheduled_run_at: 1500,
    is_running: false,
    env_keys: [],
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

describe("entriesForMissedScheduledRuns", () => {
  it("builds an unread entry for a missed scheduled fire", () => {
    const result = entriesForMissedScheduledRuns([], [routine()]);
    expect(result).toEqual([
      {
        id: "missed:r1:1500",
        routineId: "r1",
        routineTitle: "Morning review",
        message: "Scheduled run was missed — review or run manually",
        atSecs: 1500,
        read: false,
      },
    ]);
  });

  it("dedupes repeated polls for the same missed fire", () => {
    const existing: NotificationEntry[] = [
      {
        id: "missed:r1:1500",
        routineId: "r1",
        routineTitle: "Morning review",
        message: "Scheduled run was missed — review or run manually",
        atSecs: 1500,
        read: true,
      },
    ];
    expect(entriesForMissedScheduledRuns(existing, [routine()])).toBe(existing);
  });

  it("returns the same reference when there are no missed runs", () => {
    const existing: NotificationEntry[] = [];
    expect(
      entriesForMissedScheduledRuns(existing, [
        routine({ missed_scheduled_run_at: null }),
      ]),
    ).toBe(existing);
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

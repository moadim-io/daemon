import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary } from "../api/hooks";
import {
  failureNotificationText,
  fireFailureNotification,
  freshFailures,
  loadNotifyFailures,
  notificationPermission,
  notificationsSupported,
  requestNotifyPermission,
  saveNotifyFailures,
  snapshotRunStatuses,
} from "./failureNotify";

function run(overrides: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    workbench: "nightly-100",
    routine_id: "r1",
    routine_title: "Nightly build",
    started_at: 100,
    started_at_local: "1970-01-01 00:01:40",
    finished_at: 160,
    status: "failed",
    exit_code: 1,
    ...overrides,
  };
}

/** A minimal stand-in for the browser's `Notification` constructor + statics. */
class FakeNotification {
  static permission: NotificationPermission = "default";
  static requestPermission = vi.fn<() => Promise<NotificationPermission>>();
  constructor(
    public title: string,
    public options?: NotificationOptions,
  ) {}
}

const originalNotification = globalThis.Notification;

beforeEach(() => {
  localStorage.clear();
  FakeNotification.permission = "default";
  FakeNotification.requestPermission = vi.fn();
  vi.stubGlobal("Notification", FakeNotification);
});

afterEach(() => {
  vi.stubGlobal("Notification", originalNotification);
  vi.unstubAllGlobals();
});

describe("notify-failures preference", () => {
  it("defaults to off", () => {
    expect(loadNotifyFailures()).toBe(false);
  });

  it("round-trips a saved preference", () => {
    saveNotifyFailures(true);
    expect(loadNotifyFailures()).toBe(true);
    saveNotifyFailures(false);
    expect(loadNotifyFailures()).toBe(false);
  });

  // Mirrors theme.ts's equivalent guard (see theme.test.ts): both preferences are read/written
  // through the same try/catch-and-fall-back shape, so a private-mode/quota storage error must
  // not crash the app either.
  describe("when localStorage throws (private mode / quota)", () => {
    afterEach(() => {
      vi.restoreAllMocks();
    });

    it("falls back to off instead of propagating the error", () => {
      vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
        throw new DOMException("blocked", "SecurityError");
      });
      expect(loadNotifyFailures()).toBe(false);
    });

    it("saveNotifyFailures swallows the error instead of propagating it", () => {
      vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
        throw new DOMException("blocked", "SecurityError");
      });
      expect(() => saveNotifyFailures(true)).not.toThrow();
    });
  });
});

describe("notification support/permission", () => {
  it("reports supported when Notification exists", () => {
    expect(notificationsSupported()).toBe(true);
    expect(notificationPermission()).toBe("default");
  });

  it("reports unsupported when Notification is missing", () => {
    vi.stubGlobal("Notification", undefined);
    expect(notificationsSupported()).toBe(false);
    expect(notificationPermission()).toBe("unsupported");
  });
});

describe("requestNotifyPermission", () => {
  it("resolves unsupported without a Notification API", async () => {
    vi.stubGlobal("Notification", undefined);
    await expect(requestNotifyPermission()).resolves.toBe("unsupported");
  });

  it("returns the existing decision without re-prompting", async () => {
    FakeNotification.permission = "denied";
    await expect(requestNotifyPermission()).resolves.toBe("denied");
    expect(FakeNotification.requestPermission).not.toHaveBeenCalled();
  });

  it("prompts when permission is undecided", async () => {
    FakeNotification.permission = "default";
    FakeNotification.requestPermission.mockResolvedValue("granted");
    await expect(requestNotifyPermission()).resolves.toBe("granted");
    expect(FakeNotification.requestPermission).toHaveBeenCalledTimes(1);
  });
});

describe("snapshotRunStatuses / freshFailures", () => {
  it("snapshots runs keyed by workbench", () => {
    const snap = snapshotRunStatuses([run({ workbench: "a", status: "running" }), run({ workbench: "b", status: "success" })]);
    expect(snap.get("a")).toBe("running");
    expect(snap.get("b")).toBe("success");
  });

  it("flags a run that transitioned from running to failed", () => {
    const prev = snapshotRunStatuses([run({ workbench: "a", status: "running" })]);
    const now = [run({ workbench: "a", status: "failed" })];
    expect(freshFailures(now, prev)).toEqual(now);
  });

  it("flags a run that finished failed between polls with no prior entry", () => {
    const prev = snapshotRunStatuses([]);
    const now = [run({ workbench: "a", status: "failed" })];
    expect(freshFailures(now, prev)).toEqual(now);
  });

  it("does not re-flag a run already failed in the previous snapshot", () => {
    const prev = snapshotRunStatuses([run({ workbench: "a", status: "failed" })]);
    const now = [run({ workbench: "a", status: "failed" })];
    expect(freshFailures(now, prev)).toEqual([]);
  });

  it("ignores non-failed runs", () => {
    const prev = snapshotRunStatuses([]);
    const now = [run({ workbench: "a", status: "running" }), run({ workbench: "b", status: "success" })];
    expect(freshFailures(now, prev)).toEqual([]);
  });
});

describe("failureNotificationText", () => {
  it("includes the exit code when known", () => {
    expect(failureNotificationText(run({ routine_title: "Nightly build", exit_code: 2 }))).toEqual({
      title: "Nightly build failed",
      body: "Exit code 2",
    });
  });

  it("falls back to a generic body without an exit code", () => {
    expect(failureNotificationText(run({ exit_code: null }))).toEqual({
      title: "Nightly build failed",
      body: "Run failed",
    });
  });
});

describe("fireFailureNotification", () => {
  it("does nothing without granted permission", () => {
    FakeNotification.permission = "default";
    const ctorSpy = vi.fn();
    class Spy extends FakeNotification {
      constructor(title: string, options?: NotificationOptions) {
        super(title, options);
        ctorSpy(title, options);
      }
    }
    vi.stubGlobal("Notification", Spy);
    fireFailureNotification(run());
    expect(ctorSpy).not.toHaveBeenCalled();
  });

  it("constructs a Notification when permission is granted", () => {
    FakeNotification.permission = "granted";
    const ctorSpy = vi.fn();
    class Spy extends FakeNotification {
      constructor(title: string, options?: NotificationOptions) {
        super(title, options);
        ctorSpy(title, options);
      }
    }
    vi.stubGlobal("Notification", Spy);
    fireFailureNotification(run({ routine_title: "Nightly build", exit_code: 1, workbench: "nightly-100" }));
    expect(ctorSpy).toHaveBeenCalledWith("Nightly build failed", { body: "Exit code 1", tag: "nightly-100" });
  });
});

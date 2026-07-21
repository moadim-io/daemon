import type { FleetRunSummary } from "../api/hooks";

/**
 * Desktop notifications for freshly-failed runs — an ops dashboard should surface "what's
 * happening right now" without the operator having to keep a tab focused. Opt-in (avoids alert
 * fatigue) and scoped to genuine state transitions so a page load with pre-existing failures
 * already in view doesn't spam a burst of notifications.
 */

const STORAGE_KEY = "moadim.notify-failures";

/** Reads the persisted desktop-notification preference. Defaults to off (opt-in only). */
export function loadNotifyFailures(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

/** Persists the desktop-notification preference. Best-effort; ignores storage errors. */
export function saveNotifyFailures(enabled: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, enabled ? "1" : "0");
  } catch {
    // ponytail: private-mode/quota errors are non-fatal — the in-memory choice still applies this session.
  }
}

/** Whether the browser exposes the Notification API at all. */
export function notificationsSupported(): boolean {
  return typeof Notification !== "undefined";
}

/** Current permission state, or `"unsupported"` when the API itself is missing. */
export function notificationPermission(): NotificationPermission | "unsupported" {
  return notificationsSupported() ? Notification.permission : "unsupported";
}

/** Requests permission if undecided; resolves the resulting (or already-decided) state. */
export async function requestNotifyPermission(): Promise<NotificationPermission | "unsupported"> {
  if (!notificationsSupported()) return "unsupported";
  if (Notification.permission !== "default") return Notification.permission;
  return Notification.requestPermission();
}

/** A run's status at a point in time, keyed by its unique `workbench` id. */
export type RunStatusSnapshot = Map<string, FleetRunSummary["status"]>;

/** Snapshots the current runs' statuses, for diffing against a later poll. */
export function snapshotRunStatuses(runs: FleetRunSummary[]): RunStatusSnapshot {
  return new Map(runs.map((r) => [r.workbench, r.status]));
}

/**
 * Runs that are `failed` now but weren't in the previous snapshot — covers both a
 * `running`→`failed` transition and a run that finished (already failed) between polls.
 */
export function freshFailures(runs: FleetRunSummary[], previous: RunStatusSnapshot): FleetRunSummary[] {
  return runs.filter((r) => r.status === "failed" && previous.get(r.workbench) !== "failed");
}

/** Notification title/body for a freshly-failed run. */
export function failureNotificationText(run: FleetRunSummary): { title: string; body: string } {
  return {
    title: `${run.routine_title} failed`,
    body: run.exit_code == null ? "Run failed" : `Exit code ${run.exit_code}`,
  };
}

/** Fires a desktop notification for a freshly-failed run. No-op unless permission is granted. */
export function fireFailureNotification(run: FleetRunSummary): void {
  if (notificationPermission() !== "granted") return;
  const { title, body } = failureNotificationText(run);
  new Notification(title, { body, tag: run.workbench });
}

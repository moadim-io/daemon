/**
 * Persistent notification log backing the header's Notification Center: a revisitable inbox of
 * fleet-wide run failures, independent of which page is mounted. Unlike `failureNotify.ts`'s
 * opt-in desktop OS notification (Overview-only, gone the instant it's dismissed), this is
 * always-on and survives navigation/reload via `localStorage` — the standard bell-icon +
 * unread-badge + dropdown-inbox pattern (GitHub, Slack, Grafana alerting).
 */
import type { FleetRunSummary, RoutineResponse } from "../api/hooks";

export interface NotificationEntry {
  /** The failed run's `workbench` id — stable and unique, doubles as the dedupe key. */
  id: string;
  routineId: string;
  routineTitle: string;
  message: string;
  /** Unix seconds the run finished (or started, if `finished_at` is unknown). */
  atSecs: number;
  read: boolean;
}

const STORAGE_KEY = "moadim.notification-log";

/** Oldest entries beyond this are dropped — a running inbox, not an unbounded audit log. */
export const MAX_ENTRIES = 50;

function isEntry(v: unknown): v is NotificationEntry {
  if (typeof v !== "object" || v === null) return false;
  const e = v as Record<string, unknown>;
  return (
    typeof e.id === "string" &&
    typeof e.routineId === "string" &&
    typeof e.routineTitle === "string" &&
    typeof e.message === "string" &&
    typeof e.atSecs === "number" &&
    typeof e.read === "boolean"
  );
}

/** Reads the persisted log, newest-first. Empty on first run, corrupt storage, or storage errors. */
export function loadNotificationLog(): NotificationEntry[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter(isEntry) : [];
  } catch {
    return [];
  }
}

/** Persists the log. Best-effort; ignores storage errors. */
export function saveNotificationLog(entries: NotificationEntry[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } catch {
    // ponytail: private-mode/quota errors are non-fatal — the in-memory log still applies this session.
  }
}

/**
 * Prepends one entry per freshly-failed run (as reported by `failureNotify.ts`'s `freshFailures`)
 * not already present, newest-first, capped at `MAX_ENTRIES`. Returns `existing` unchanged
 * (same reference) when there's nothing new, so callers can skip a state update.
 */
export function entriesForFailures(
  existing: NotificationEntry[],
  failed: FleetRunSummary[],
): NotificationEntry[] {
  const known = new Set(existing.map((e) => e.id));
  const fresh: NotificationEntry[] = failed
    .filter((r) => !known.has(r.workbench))
    .map((r) => ({
      id: r.workbench,
      routineId: r.routine_id,
      routineTitle: r.routine_title,
      message: r.exit_code == null ? "Run failed" : `Run failed (exit ${r.exit_code})`,
      atSecs: r.finished_at ?? r.started_at,
      read: false,
    }));
  if (fresh.length === 0) return existing;
  return [...fresh, ...existing].slice(0, MAX_ENTRIES);
}

function missedEntryId(routine: RoutineResponse): string | undefined {
  return routine.missed_scheduled_run_at == null
    ? undefined
    : `missed:${routine.id}:${routine.missed_scheduled_run_at}`;
}

/**
 * Prepends one entry per routine with an alert-only missed scheduled fire. Unlike failed-run
 * notifications, this intentionally surfaces already-present misses on page load because no run
 * record exists elsewhere. Existing IDs dedupe repeated polls until the routine records a later
 * scheduled trigger or the operator clears the inbox.
 */
export function entriesForMissedScheduledRuns(
  existing: NotificationEntry[],
  routines: RoutineResponse[],
): NotificationEntry[] {
  const known = new Set(existing.map((e) => e.id));
  const fresh: NotificationEntry[] = routines.flatMap((routine) => {
    const id = missedEntryId(routine);
    if (id === undefined || known.has(id)) return [];
    return [
      {
        id,
        routineId: routine.id,
        routineTitle: routine.title,
        message: "Scheduled run was missed — review or run manually",
        atSecs: routine.missed_scheduled_run_at ?? 0,
        read: false,
      },
    ];
  });
  if (fresh.length === 0) return existing;
  fresh.sort((a, b) => b.atSecs - a.atSecs || a.routineTitle.localeCompare(b.routineTitle));
  return [...fresh, ...existing].slice(0, MAX_ENTRIES);
}

/** Unread entry count, for the bell's badge. */
export function unreadCount(entries: NotificationEntry[]): number {
  return entries.filter((e) => !e.read).length;
}

/** Marks every entry read. Returns `entries` unchanged (same reference) when already all-read. */
export function markAllRead(entries: NotificationEntry[]): NotificationEntry[] {
  return entries.some((e) => !e.read) ? entries.map((e) => ({ ...e, read: true })) : entries;
}

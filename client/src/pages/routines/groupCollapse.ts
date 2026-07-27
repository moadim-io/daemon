/**
 * Persists which group-by sections are collapsed in the routines table, keyed by
 * `${groupBy}:${label}` so switching group-by dimensions can't collide two same-named groups
 * (e.g. a "Disabled" status group and an agent literally named "Disabled").
 */
const KEY = "moadim.routines.collapsedGroups";

/** Reads the persisted collapsed-group key set. Empty on first load or storage failure. */
export function loadCollapsedGroups(): Set<string> {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return new Set();
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? new Set(parsed.filter((v): v is string => typeof v === "string")) : new Set();
  } catch {
    return new Set();
  }
}

/** Persists the collapsed-group key set (best-effort; ignores storage errors). */
export function saveCollapsedGroups(groups: ReadonlySet<string>): void {
  try {
    localStorage.setItem(KEY, JSON.stringify([...groups]));
  } catch {
    // storage unavailable (private mode / quota) — in-memory collapse state still applies
  }
}

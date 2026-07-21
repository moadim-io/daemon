/**
 * Persisted "saved views" for the Routines page: the current filter, sort, and group-by state
 * can be captured into a portable `ViewSnapshot`, saved under a name, and re-applied later. The
 * most recent state is auto-persisted and restored on load so a reload doesn't silently drop an
 * operator's in-progress triage view (Linear/GitHub Issues/Grafana pinned-view convention).
 * Direct port of `ui/src/routines/saved_views.rs (removed)`.
 */
import {
  namedFacetValue,
  machineFacetValue,
  parseMachineFacet,
  parseNamedFacet,
  parseStatusFacet,
  type RoutineFilter,
} from "./filter";
import { parseRCol, parseRDir, parseRGroupBy, type RCol, type RDir, type RGroupBy } from "./routineState";

const SAVED_VIEWS_KEY = "moadim.routines.saved_views";
const LAST_VIEW_KEY = "moadim.routines.last_view";

/**
 * Portable snapshot of the Routines page's filter, sort, and group-by state. Every field is a
 * plain string token, so this round-trips through JSON without depending on internal
 * representations.
 */
export interface ViewSnapshot {
  query: string;
  status: string;
  agent: string;
  machine: string;
  repository: string;
  tag: string;
  sortCol: string | undefined;
  sortDir: string;
  groupBy: string;
}

/** Capture the given filter/sort/group-by state into a snapshot. */
export function captureSnapshot(
  filter: RoutineFilter,
  sortCol: RCol | undefined,
  sortDir: RDir,
  groupBy: RGroupBy,
): ViewSnapshot {
  return {
    query: filter.query,
    status: filter.status,
    agent: namedFacetValue(filter.agent),
    machine: machineFacetValue(filter.machine),
    repository: namedFacetValue(filter.repository),
    tag: namedFacetValue(filter.tag),
    sortCol,
    sortDir,
    groupBy,
  };
}

/**
 * Decode a snapshot back into live filter/sort/group-by state. Unknown or missing tokens fall
 * back to each facet's default, so a snapshot from an older build (or hand-edited storage)
 * degrades gracefully instead of failing to load.
 */
export function decodeSnapshot(
  snapshot: ViewSnapshot,
): { filter: RoutineFilter; sortCol: RCol | undefined; sortDir: RDir; groupBy: RGroupBy } {
  const filter: RoutineFilter = {
    query: snapshot.query,
    status: parseStatusFacet(snapshot.status),
    agent: parseNamedFacet(snapshot.agent),
    machine: parseMachineFacet(snapshot.machine),
    repository: parseNamedFacet(snapshot.repository),
    tag: parseNamedFacet(snapshot.tag),
  };
  const sortCol = snapshot.sortCol === undefined ? undefined : parseRCol(snapshot.sortCol);
  const sortDir = parseRDir(snapshot.sortDir);
  const groupBy = parseRGroupBy(snapshot.groupBy);
  return { filter, sortCol, sortDir, groupBy };
}

/** A named, saved filter/sort/group-by preset. */
export interface SavedView {
  name: string;
  snapshot: ViewSnapshot;
}

function readJson<T>(key: string): T | undefined {
  try {
    const raw = window.localStorage.getItem(key);
    return raw === null ? undefined : (JSON.parse(raw) as T);
  } catch {
    return undefined;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Best-effort: a storage error (e.g. private-mode quota) is silently ignored — the
    // in-memory value still applies for the session.
  }
}

/** Load the saved-view list, defaulting to empty when storage is unavailable/garbage. */
export function loadSavedViews(): SavedView[] {
  return readJson<SavedView[]>(SAVED_VIEWS_KEY) ?? [];
}

/** Persist the saved-view list. Best-effort. */
export function saveSavedViews(views: SavedView[]): void {
  writeJson(SAVED_VIEWS_KEY, views);
}

/** Load the last-applied filter/sort/group-by snapshot, if any was persisted. */
export function loadLastView(): ViewSnapshot | undefined {
  return readJson<ViewSnapshot>(LAST_VIEW_KEY);
}

/** Persist the current filter/sort/group-by snapshot as the one to restore on next load. */
export function saveLastView(snapshot: ViewSnapshot): void {
  writeJson(LAST_VIEW_KEY, snapshot);
}

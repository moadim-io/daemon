/**
 * URL query-param encoding for the Routines page's filter/sort/group-by state, so the current
 * view is reflected in `location.search` and can be shared as a link (CI dashboards, issue
 * trackers, and observability tools all support this — a filtered view is only as useful as it
 * is shareable). Reuses the same `ViewSnapshot` shape as the localStorage-backed "saved views"
 * feature (see `savedViews.ts`); this module only adds a second encoding for it.
 */
import { defaultRoutineFilter } from "./filter";
import { captureSnapshot, type ViewSnapshot } from "./savedViews";

const DEFAULT_SNAPSHOT = captureSnapshot(defaultRoutineFilter(), undefined, "asc", "none");

const PARAM_KEYS = {
  query: "q",
  status: "status",
  agent: "agent",
  machine: "machine",
  repository: "repo",
  tag: "tag",
  sortCol: "sort",
  sortDir: "dir",
  groupBy: "group",
} as const satisfies Record<keyof ViewSnapshot, string>;

/** The URL param names this module owns — used to clear stale view params before re-applying. */
export const VIEW_PARAM_NAMES: readonly string[] = Object.values(PARAM_KEYS);

/**
 * Encode a snapshot into URL query params, omitting any field that's at its default value so an
 * unfiltered view keeps a clean URL (only narrowed/sorted/grouped state shows up as `?...`).
 */
export function snapshotToParams(snapshot: ViewSnapshot): URLSearchParams {
  const params = new URLSearchParams();
  for (const key of Object.keys(PARAM_KEYS) as (keyof ViewSnapshot)[]) {
    const value = snapshot[key];
    if (value === undefined || value === DEFAULT_SNAPSHOT[key]) continue;
    params.set(PARAM_KEYS[key], value);
  }
  return params;
}

/**
 * Merge a snapshot's params into an existing `URLSearchParams`, replacing whatever view params
 * were there before while leaving unrelated params (e.g. a future deep-link key) untouched.
 */
export function applyViewToParams(prev: URLSearchParams, snapshot: ViewSnapshot): URLSearchParams {
  const next = new URLSearchParams(prev);
  for (const name of VIEW_PARAM_NAMES) next.delete(name);
  for (const [k, v] of snapshotToParams(snapshot)) next.set(k, v);
  return next;
}

/**
 * Decode a snapshot from URL query params, or `undefined` if none of the recognized keys are
 * present — so a bare `/routines` URL doesn't override the locally-persisted last view.
 */
export function paramsToSnapshot(params: URLSearchParams): ViewSnapshot | undefined {
  if (!VIEW_PARAM_NAMES.some((name) => params.has(name))) return undefined;
  return {
    query: params.get(PARAM_KEYS.query) ?? DEFAULT_SNAPSHOT.query,
    status: params.get(PARAM_KEYS.status) ?? DEFAULT_SNAPSHOT.status,
    agent: params.get(PARAM_KEYS.agent) ?? DEFAULT_SNAPSHOT.agent,
    machine: params.get(PARAM_KEYS.machine) ?? DEFAULT_SNAPSHOT.machine,
    repository: params.get(PARAM_KEYS.repository) ?? DEFAULT_SNAPSHOT.repository,
    tag: params.get(PARAM_KEYS.tag) ?? DEFAULT_SNAPSHOT.tag,
    sortCol: params.get(PARAM_KEYS.sortCol) ?? undefined,
    sortDir: params.get(PARAM_KEYS.sortDir) ?? DEFAULT_SNAPSHOT.sortDir,
    groupBy: params.get(PARAM_KEYS.groupBy) ?? DEFAULT_SNAPSHOT.groupBy,
  };
}

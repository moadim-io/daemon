/**
 * Page/view/sort/group-by state helpers. Direct port of the pure parts of
 * `ui/src/routines/state.rs` (the `RState`/`RAction` reducer itself is reimplemented as plain
 * React `useState` in `RoutinesPage`, per the TanStack Query conventions used elsewhere in this
 * app — see the module doc in `page.tsx`).
 */
import type { RoutineResponse } from "../../api/hooks";
import { nextFireAfter } from "../../lib/schedule";
import { lastFireAt, routineHealth, healthPriority, healthBadge } from "./filter";

// ─── Sort column / direction ─────────────────────────────────────────────────

export type RCol = "title" | "next_run" | "last_fire" | "agent" | "health" | "enabled" | "updated";

const RCOLS: readonly RCol[] = [
  "title",
  "next_run",
  "last_fire",
  "agent",
  "health",
  "enabled",
  "updated",
];

/** Parse a persisted token back to a column. `undefined` for unrecognized tokens. */
export function parseRCol(s: string): RCol | undefined {
  return (RCOLS as readonly string[]).includes(s) ? (s as RCol) : undefined;
}

export type RDir = "asc" | "desc";

export function flipDir(d: RDir): RDir {
  return d === "asc" ? "desc" : "asc";
}

/** Parse a persisted token back to a direction, defaulting to `"asc"` for unknown values. */
export function parseRDir(s: string): RDir {
  return s === "desc" ? "desc" : "asc";
}

// ─── Group-by ────────────────────────────────────────────────────────────────

export type RGroupBy = "none" | "agent" | "machine" | "status" | "health";

const RGROUP_BYS: readonly RGroupBy[] = ["none", "agent", "machine", "status", "health"];

/** Parse a persisted token back to a variant, defaulting to `"none"` for unknown values. */
export function parseRGroupBy(s: string): RGroupBy {
  return (RGROUP_BYS as readonly string[]).includes(s) ? (s as RGroupBy) : "none";
}

/** Short human label shown in the group-by selector. */
export function groupByLabel(by: RGroupBy): string {
  switch (by) {
    case "none":
      return "None";
    case "agent":
      return "Agent";
    case "machine":
      return "Machine";
    case "status":
      return "Status";
    case "health":
      return "Health";
  }
}

/** Group key for a single routine under the given dimension. */
export function routineGroupKey(r: RoutineResponse, by: RGroupBy): string {
  switch (by) {
    case "none":
      return "";
    case "agent":
      return r.agent;
    case "machine":
      return r.machines?.[0] ?? "(unassigned)";
    case "status":
      return r.enabled ? "Enabled" : "Disabled";
    case "health":
      return healthBadge(routineHealth(r, new Date()));
  }
}

/**
 * Partition `routines` into `(groupLabel, routinesInGroup)` pairs sorted alphabetically by
 * label. Within each group the input order is preserved. When `by` is `"none"`, returns a single
 * pair with an empty label.
 */
export function groupRoutines(
  routines: RoutineResponse[],
  by: RGroupBy,
): [string, RoutineResponse[]][] {
  if (by === "none") return [["", routines]];
  const map = new Map<string, RoutineResponse[]>();
  for (const r of routines) {
    const key = routineGroupKey(r, by);
    const bucket = map.get(key);
    if (bucket) bucket.push(r);
    else map.set(key, [r]);
  }
  return [...map.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
}

/**
 * Return `routines` sorted by `col` in `dir` order. When `col` is `undefined` the input order is
 * preserved. Ties break by id for a stable sort.
 */
export function sortRoutines(
  routines: RoutineResponse[],
  col: RCol | undefined,
  dir: RDir,
  now: Date,
): RoutineResponse[] {
  if (col === undefined) return routines;
  const cmp = (a: RoutineResponse, b: RoutineResponse): number => {
    let primary: number;
    switch (col) {
      case "title":
        primary = a.title.toLowerCase().localeCompare(b.title.toLowerCase());
        break;
      case "agent":
        primary = a.agent.toLowerCase().localeCompare(b.agent.toLowerCase());
        break;
      case "enabled":
        primary = Number(a.enabled) - Number(b.enabled);
        break;
      case "updated":
        primary = a.updated_at - b.updated_at;
        break;
      case "health":
        primary =
          healthPriority(routineHealth(a, now)) - healthPriority(routineHealth(b, now));
        break;
      case "last_fire": {
        const fa = lastFireAt(a);
        const fb = lastFireAt(b);
        primary = (fa ?? -1) - (fb ?? -1);
        break;
      }
      case "next_run": {
        const nextOf = (r: RoutineResponse): number | undefined =>
          r.enabled ? nextFireAfter(r.schedule, now)?.getTime() : undefined;
        const na = nextOf(a);
        const nb = nextOf(b);
        if (na !== undefined && nb !== undefined) primary = na - nb;
        else if (na !== undefined) primary = -1;
        else if (nb !== undefined) primary = 1;
        else primary = 0;
        break;
      }
    }
    const directed = dir === "desc" ? -primary : primary;
    return directed !== 0 ? directed : a.id.localeCompare(b.id);
  };
  return [...routines].sort(cmp);
}

import type { Flag, RoutineResponse } from "../../api/hooks";

/** One open flag joined with the routine it was raised against. */
export interface FlagRow {
  routineId: string;
  routineTitle: string;
  flag: Flag;
}

/** Flattens per-routine flag lists (aligned with `routines` by index) into inbox rows, oldest first. */
export function buildFlagRows(
  routines: readonly RoutineResponse[],
  lists: readonly (readonly Flag[] | undefined)[],
): FlagRow[] {
  return routines
    .flatMap((r, i) => (lists[i] ?? []).map((flag) => ({ routineId: r.id, routineTitle: r.title, flag })))
    .sort((a, b) => a.flag.created_at - b.flag.created_at);
}

/** Case-insensitive text match over routine/type/description, plus an exact type filter ("" = any). */
export function filterFlagRows(rows: readonly FlagRow[], query: string, type: string): FlagRow[] {
  const q = query.trim().toLowerCase();
  return rows.filter(
    (r) =>
      (type === "" || r.flag.type === type) &&
      (q === "" || `${r.routineTitle} ${r.flag.type} ${r.flag.description}`.toLowerCase().includes(q)),
  );
}

/** Sorted distinct flag types, for the type filter. */
export function flagTypes(rows: readonly FlagRow[]): string[] {
  return [...new Set(rows.map((r) => r.flag.type))].sort();
}

/** Stat-card numbers: open flags, distinct routines flagged, oldest flag's `created_at` (null if none). */
export function flagStats(rows: readonly FlagRow[]) {
  return {
    open: rows.length,
    routines: new Set(rows.map((r) => r.routineId)).size,
    oldestAt: rows.length === 0 ? null : Math.min(...rows.map((r) => r.flag.created_at)),
  };
}

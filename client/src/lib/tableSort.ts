/**
 * Client-side column sorting for the fleet tables (Runs, Reliability).
 *
 * Best practice (data-table guidance from Carbon / Atlassian / WAI-ARIA APG "Sortable Table"):
 * every column an operator compares on is sortable from its header, the header is a real
 * button so the keyboard can reach it, the active column exposes `aria-sort`, missing values
 * always sink to the bottom regardless of direction, and the chosen sort survives a reload.
 * A third click clears the sort and restores the table's natural (ranked / newest-first) order.
 */
import { useState } from "react";

export type SortDir = "asc" | "desc";

export interface SortState<K extends string> {
  key: K;
  dir: SortDir;
}

/** Per-column sort value; `null` means "no value" and always sorts last. */
export type SortAccessors<T, K extends string> = Record<K, (row: T) => number | string | null>;

/** Header-click transition: new column → its default dir, same column → flip, then → cleared. */
export function nextSort<K extends string>(
  cur: SortState<K> | null,
  key: K,
  defaultDir: SortDir,
): SortState<K> | null {
  if (cur === null || cur.key !== key) return { key, dir: defaultDir };
  if (cur.dir === defaultDir) return { key, dir: defaultDir === "asc" ? "desc" : "asc" };
  return null;
}

/** Stable sort of `rows` by `state`; `null` state returns the input order unchanged. */
export function sortRows<T, K extends string>(
  rows: readonly T[],
  state: SortState<K> | null,
  accessors: SortAccessors<T, K>,
): T[] {
  if (state === null) return [...rows];
  const get = accessors[state.key];
  const sign = state.dir === "asc" ? 1 : -1;
  return rows
    .map((row, i) => ({ row, i, v: get(row) }))
    .sort((a, b) => {
      if (a.v === null || b.v === null) {
        if (a.v === b.v) return a.i - b.i;
        return a.v === null ? 1 : -1;
      }
      const c =
        typeof a.v === "number" && typeof b.v === "number"
          ? a.v - b.v
          : String(a.v).localeCompare(String(b.v), undefined, { sensitivity: "base", numeric: true });
      return c === 0 ? a.i - b.i : c * sign;
    })
    .map((x) => x.row);
}

/** Reads a persisted sort, discarding anything malformed or naming an unknown column. */
export function loadSort<K extends string>(storageKey: string, keys: readonly K[]): SortState<K> | null {
  try {
    const raw = localStorage.getItem(storageKey);
    if (raw === null) return null;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return null;
    const { key, dir } = parsed as { key?: unknown; dir?: unknown };
    if (!keys.includes(key as K) || (dir !== "asc" && dir !== "desc")) return null;
    return { key: key as K, dir };
  } catch {
    return null;
  }
}

/** Persists (or, for `null`, clears) a sort — best-effort, storage errors are ignored. */
export function saveSort<K extends string>(storageKey: string, state: SortState<K> | null): void {
  try {
    if (state === null) localStorage.removeItem(storageKey);
    else localStorage.setItem(storageKey, JSON.stringify(state));
  } catch {
    // storage unavailable (private mode / quota) — in-memory sort still applies
  }
}

/** Sort state backed by localStorage under `storageKey`; `toggle` applies `nextSort`. */
export function usePersistedSort<K extends string>(storageKey: string, keys: readonly K[]) {
  const [sort, setSort] = useState<SortState<K> | null>(() => loadSort(storageKey, keys));
  const toggle = (key: K, defaultDir: SortDir) => {
    const next = nextSort(sort, key, defaultDir);
    saveSort(storageKey, next);
    setSort(next);
  };
  return { sort, toggle };
}

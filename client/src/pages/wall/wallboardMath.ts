/**
 * Pure tile math for the Wallboard (kiosk / TV mode) page: one tile per routine, its health
 * state derived from the fleet's recent runs plus the Overview's attention rules, sorted
 * worst-first so the problems sit at the top-left of the screen.
 */
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { nextFireAfterAny, scheduleList } from "../../lib/schedule";
import { ATTENTION_BADGE, attentionReason, fromRoutine } from "../overview/overviewLogic";

export type TileState = "failing" | "attention" | "running" | "ok" | "idle" | "snoozed";

/** Worst-first display order; also the order of the summary counts. */
export const TILE_STATES: readonly TileState[] = ["failing", "attention", "running", "ok", "idle", "snoozed"];

export const TILE_LABEL: Record<TileState, string> = {
  failing: "FAILING",
  attention: "ATTENTION",
  running: "RUNNING",
  ok: "OK",
  idle: "NO RUNS",
  snoozed: "SNOOZED",
};

/** How many recent outcomes each tile's history strip shows. */
export const HISTORY_LEN = 10;

export interface WallTile {
  id: string;
  title: string;
  state: TileState;
  /** One-line why, e.g. "last run failed · exit 2". */
  reason: string;
  /** Recent run statuses, oldest → newest. */
  history: FleetRunSummary["status"][];
  /** Newest run, for the click-through link. */
  latest?: FleetRunSummary;
  /** Unix seconds of the newest run start. */
  lastStartedAt?: number;
  /** Next scheduled fire, when enabled, not snoozed and the schedule has one. */
  nextAt?: Date;
}

function runsByRoutine(runs: FleetRunSummary[]): Map<string, FleetRunSummary[]> {
  const map = new Map<string, FleetRunSummary[]>();
  for (const run of runs) {
    const list = map.get(run.routine_id) ?? [];
    list.push(run);
    map.set(run.routine_id, list);
  }
  for (const list of map.values()) list.sort((a, b) => b.started_at - a.started_at);
  return map;
}

/**
 * One tile per routine, worst state first, ties by title. Manually disabled routines are left
 * out (intentional, not a fault); auto-disabled ones stay, since the circuit breaker tripping is
 * exactly what a wallboard must show. `runs` may be any newest-first-or-not fleet slice.
 */
export function buildTiles(routines: RoutineResponse[], runs: FleetRunSummary[], now: Date): WallTile[] {
  const byRoutine = runsByRoutine(runs);
  const tiles: WallTile[] = [];
  for (const routine of routines) {
    const source = fromRoutine(routine, now);
    if (!source.enabled && source.autoDisabledReason === undefined) continue;
    const list = byRoutine.get(routine.id) ?? [];
    const latest = list[0];
    const lastFinished = list.find((r) => r.status !== "running");
    const fault = attentionReason(source, now);

    let state: TileState;
    let reason: string;
    if (list.some((r) => r.status === "running")) {
      state = "running";
      reason = "run in progress";
    } else if (lastFinished?.status === "failed") {
      state = "failing";
      reason = lastFinished.exit_code == null ? "last run failed" : `last run failed · exit ${lastFinished.exit_code}`;
    } else if (fault !== undefined) {
      state = "attention";
      reason = ATTENTION_BADGE[fault].toLowerCase();
    } else if (source.snoozed) {
      state = "snoozed";
      reason = "scheduled fires suppressed";
    } else if (latest === undefined) {
      state = "idle";
      reason = "no runs yet";
    } else {
      state = "ok";
      reason = "last run succeeded";
    }

    tiles.push({
      id: routine.id,
      title: routine.title,
      state,
      reason,
      history: list.slice(0, HISTORY_LEN).map((r) => r.status).reverse(),
      latest,
      lastStartedAt: latest?.started_at,
      nextAt: source.enabled && !source.snoozed ? nextFireAfterAny(scheduleList(source), now) : undefined,
    });
  }
  const rank = (s: TileState) => TILE_STATES.indexOf(s);
  tiles.sort((a, b) => rank(a.state) - rank(b.state) || a.title.localeCompare(b.title));
  return tiles;
}

/** Tile count per state, every state present (zero when none). */
export function tileCounts(tiles: WallTile[]): Record<TileState, number> {
  const counts = Object.fromEntries(TILE_STATES.map((s) => [s, 0])) as Record<TileState, number>;
  for (const t of tiles) counts[t.state] += 1;
  return counts;
}

/**
 * Pure aggregation logic for the Overview page: KPI tile counts, the merged
 * soonest-first "upcoming runs" list, the "needs attention" triage
 * classification, and the record -> `SchedSource` mappers.
 *
 * Direct TS port of `ui/src/overview.rs` + `ui/src/overview_attention.rs`'s
 * pure functions (no DOM/fetch here — see `OverviewPage.tsx` for the data
 * loading shell). All functions take `now` explicitly so they stay
 * deterministic and unit-testable (see `overviewLogic.test.ts`).
 */
import type { RoutineResponse } from "../../api/hooks";
import { firesWithin, nextFireAfter, fmtUntil } from "../../lib/schedule";

/** Which kind of scheduled entity a row/tile refers to. Only "routine" exists today. */
export type Kind = "routine";

/** "Due soon" window: an enabled entity firing within this many ms is operationally urgent. */
export const DUE_SOON_WINDOW_MS = 3_600_000;

/** How many of the soonest upcoming runs the merged timeline shows. */
export const UPCOMING_LIMIT = 8;

/** A schedule-bearing entity reduced to just what the overview math needs. */
export interface SchedSource {
  kind: Kind;
  /** API id used to trigger the entity (routine UUID). */
  id: string;
  /** Display name: the routine title. */
  label: string;
  /** Raw cron expression used to compute the next fire. */
  schedule: string;
  /** Server-provided human description of the schedule, when present. */
  human: string | undefined;
  /** Whether the entity is currently enabled (disabled ones never fire). */
  enabled: boolean;
  /** `true` when the entity targets no machine (empty or all-blank list). */
  machinesEmpty: boolean;
  /** Whether the routine's agent is registered. */
  agentRegistered: boolean;
  /** Number of open flags raised against this entity. */
  flagCount: number;
  /** Whether scheduled fires are currently suppressed (snoozed or skip-runs active). */
  snoozed: boolean;
}

/** Aggregate counts shown as the KPI tile row. */
export interface Kpis {
  total: number;
  enabled: number;
  disabled: number;
  dueSoon: number;
  attention: number;
  flags: number;
  snoozed: number;
  dormant: number;
}

/** One entry in the merged upcoming-runs timeline. */
export interface UpcomingRun {
  kind: Kind;
  id: string;
  label: string;
  human: string | undefined;
  schedule: string;
  at: Date;
  soon: boolean;
  flagCount: number;
}

/** Why an enabled entity needs attention, in triage priority order (lower = higher priority). */
export type AttentionReason = "dormant" | "dead-schedule" | "agent-unregistered" | "has-open-flags";

const ATTENTION_RANK: Record<AttentionReason, number> = {
  dormant: 0,
  "dead-schedule": 1,
  "agent-unregistered": 2,
  "has-open-flags": 3,
};

/** Short uppercase badge label for the ISSUE column. */
export const ATTENTION_BADGE: Record<AttentionReason, string> = {
  dormant: "DORMANT",
  "dead-schedule": "DEAD SCHEDULE",
  "agent-unregistered": "AGENT MISSING",
  "has-open-flags": "OPEN FLAGS",
};

/** Human explanation of the operational consequence. */
export const ATTENTION_DETAIL: Record<AttentionReason, string> = {
  dormant: "assigned to no machine — fires nowhere",
  "dead-schedule": "schedule has no future fire — never runs again",
  "agent-unregistered": "agent not registered — every run errors",
  "has-open-flags": "agent raised flags during a run — needs review",
};

/** One enabled-but-misconfigured entity surfaced in the NEEDS ATTENTION panel. */
export interface AttentionItem {
  kind: Kind;
  label: string;
  reason: AttentionReason;
  /** Open flag count; non-zero only when `reason === "has-open-flags"`. */
  flagCount: number;
}

/** Ordinal (byte-order) string comparison, matching Rust's `Ord for String`. */
function compareLabel(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * The single most fundamental fault for an enabled `source`, or `undefined`
 * when it is healthy. Disabled entities are intentional and never flagged.
 * Faults are checked in priority order so each entity reports exactly one
 * reason.
 */
export function attentionReason(source: SchedSource, now: Date): AttentionReason | undefined {
  if (!source.enabled) return undefined;
  if (source.machinesEmpty) return "dormant";
  if (nextFireAfter(source.schedule, now) === undefined) return "dead-schedule";
  if (source.agentRegistered === false) return "agent-unregistered";
  if (source.flagCount > 0) return "has-open-flags";
  return undefined;
}

/** All enabled-but-misconfigured entities, worst fault first, ties broken by label. */
export function attentionItems(sources: SchedSource[], now: Date): AttentionItem[] {
  const items: AttentionItem[] = [];
  for (const s of sources) {
    const reason = attentionReason(s, now);
    if (reason === undefined) continue;
    items.push({ kind: s.kind, label: s.label, reason, flagCount: s.flagCount });
  }
  items.sort((a, b) => ATTENTION_RANK[a.reason] - ATTENTION_RANK[b.reason] || compareLabel(a.label, b.label));
  return items;
}

/** Count the KPI tiles from `sources` as of `now`. */
export function computeKpis(sources: SchedSource[], now: Date): Kpis {
  const total = sources.length;
  const enabled = sources.filter((s) => s.enabled).length;
  const dueSoon = sources.filter(
    (s) => s.enabled && !s.snoozed && firesWithin(s.schedule, now, DUE_SOON_WINDOW_MS),
  ).length;
  const flags = sources.reduce((sum, s) => sum + s.flagCount, 0);
  const snoozed = sources.filter((s) => s.enabled && s.snoozed).length;
  const dormant = sources.filter((s) => s.enabled && s.machinesEmpty).length;
  return {
    total,
    enabled,
    disabled: total - enabled,
    dueSoon,
    attention: attentionItems(sources, now).length,
    flags,
    snoozed,
    dormant,
  };
}

/**
 * The merged, soonest-first list of the next `UPCOMING_LIMIT` fires across
 * every enabled, non-snoozed source. Disabled, snoozed, and ones with no
 * valid future fire are dropped; ties on fire time break by label.
 */
export function upcomingRuns(sources: SchedSource[], now: Date): UpcomingRun[] {
  const runs: UpcomingRun[] = [];
  for (const s of sources) {
    if (!s.enabled || s.snoozed) continue;
    const at = nextFireAfter(s.schedule, now);
    if (at === undefined) continue;
    runs.push({
      kind: s.kind,
      id: s.id,
      label: s.label,
      human: s.human,
      schedule: s.schedule,
      at,
      soon: at.getTime() - now.getTime() <= DUE_SOON_WINDOW_MS,
      flagCount: s.flagCount,
    });
  }
  runs.sort((a, b) => a.at.getTime() - b.at.getTime() || compareLabel(a.label, b.label));
  return runs.slice(0, UPCOMING_LIMIT);
}

/** Short relative countdown to the very next fire, e.g. "in 4m", or `undefined` when nothing is scheduled. */
export function nextRunSummary(runs: UpcomingRun[], now: Date): string | undefined {
  return runs[0] === undefined ? undefined : fmtUntil(now, runs[0].at);
}

/** `true` when no entry names a real machine: an empty list, or one holding only blank entries. */
function targetsNoMachine(machines: string[] | undefined): boolean {
  return (machines ?? []).every((m) => m.trim() === "");
}

/** `true` when scheduled fires are currently suppressed (snoozed-until in the future, or skip-runs active). */
function isSnoozed(routine: RoutineResponse, now: Date): boolean {
  const untilSuppressed =
    routine.snoozed_until != null && routine.snoozed_until > Math.floor(now.getTime() / 1000);
  const skipSuppressed = routine.skip_runs != null && routine.skip_runs > 0;
  return untilSuppressed || skipSuppressed;
}

/**
 * Map a routine onto the shared schedule abstraction. Takes `now` explicitly
 * (rather than sampling the wall clock here) so this stays a pure,
 * host-testable function in lockstep with the rest of the page's math.
 */
export function fromRoutine(routine: RoutineResponse, now: Date): SchedSource {
  return {
    kind: "routine",
    id: routine.id,
    label: routine.title,
    schedule: routine.schedule,
    human: routine.schedule_description ?? undefined,
    enabled: routine.enabled,
    machinesEmpty: targetsNoMachine(routine.machines),
    agentRegistered: routine.agent_registered,
    flagCount: routine.flag_count,
    snoozed: isSnoozed(routine, now),
  };
}

/** Map the routine record list into one `SchedSource` array. */
export function sourcesOf(routines: RoutineResponse[], now: Date): SchedSource[] {
  return routines.map((r) => fromRoutine(r, now));
}

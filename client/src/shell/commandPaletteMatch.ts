import type { RoutineResponse } from "../api/hooks";

/** What a palette entry points at. */
export type CmdKind =
  | "nav-overview"
  | "nav-routines"
  | "nav-heatmap"
  | "nav-reliability"
  | "nav-machines"
  | "nav-settings"
  | "routine"
  | "action-refresh"
  | "action-stop"
  | "action-toggle-theme";

export type RouteKind = "home" | "routines" | "heatmap" | "reliability" | "machines" | "settings";

export interface Command {
  kind: CmdKind;
  title: string;
  subtitle: string;
  /** Extra terms folded into the fuzzy haystack (agent, schedule, id, tags…). */
  keywords: string;
  /** Present only for `kind: "routine"` commands. */
  routineId?: string;
}

/** The page a command navigates to, or `undefined` for an action command. */
export function routeFor(kind: CmdKind): RouteKind | undefined {
  switch (kind) {
    case "nav-overview":
      return "home";
    case "nav-routines":
    case "routine":
      return "routines";
    case "nav-heatmap":
      return "heatmap";
    case "nav-reliability":
      return "reliability";
    case "nav-machines":
      return "machines";
    case "nav-settings":
      return "settings";
    default:
      return undefined;
  }
}

/** Short category badge text for a command. */
export function badgeFor(kind: CmdKind): string {
  switch (kind) {
    case "nav-overview":
    case "nav-routines":
    case "nav-heatmap":
    case "nav-reliability":
    case "nav-machines":
    case "nav-settings":
      return "GO";
    case "routine":
      return "ROUTINE";
    default:
      return "ACTION";
  }
}

/**
 * Fuzzy-scores `query` as an ordered subsequence of `text` (case-insensitive).
 * Returns `undefined` when `query` isn't a subsequence of `text`; a blank
 * query matches everything with a neutral score of `0`. Bonuses reward a hit
 * at the start of the text, a hit after a word boundary, and consecutive
 * runs; longer texts are mildly penalized.
 */
export function fuzzyScore(text: string, query: string): number | undefined {
  const needle = query.trim();
  if (needle === "") return 0;
  const hay = [...text.toLowerCase()];
  const pins = [...needle.toLowerCase()];
  let score = 0;
  let cursor = 0;
  let prev: number | undefined;
  for (const needleCh of pins) {
    let hit: number | undefined;
    while (cursor < hay.length) {
      const hayCh = hay[cursor];
      cursor += 1;
      if (hayCh === needleCh) {
        hit = cursor - 1;
        break;
      }
    }
    if (hit === undefined) return undefined;
    score += 1;
    if (hit === 0) {
      score += 10;
    } else if (!/[a-z0-9]/i.test(hay[hit - 1] ?? "")) {
      score += 6;
    }
    if (prev !== undefined && hit === prev + 1) {
      score += 8;
    }
    prev = hit;
  }
  score -= Math.floor(hay.length / 16);
  return score;
}

function commandScore(command: Command, query: string): number | undefined {
  const title = fuzzyScore(command.title, query);
  const aliasRaw = fuzzyScore(command.keywords, query);
  const alias = aliasRaw === undefined ? undefined : aliasRaw - 4;
  if (title !== undefined && alias !== undefined) return Math.max(title, alias);
  return title ?? alias;
}

/** Indices of `commands` matching `query`, best-first; ties keep input order. */
export function rank(commands: Command[], query: string): number[] {
  const scored: Array<[number, number]> = [];
  commands.forEach((command, idx) => {
    const score = commandScore(command, query);
    if (score !== undefined) scored.push([idx, score]);
  });
  scored.sort(([aIdx, aScore], [bIdx, bScore]) => bScore - aScore || aIdx - bIdx);
  return scored.map(([idx]) => idx);
}

/** The human schedule description when present, else the raw expression, else a dash. */
export function scheduleLabel(human: string | null | undefined, raw: string): string {
  if (human) return human;
  return raw.trim() !== "" ? raw : "—";
}

/** Subtitle for a routine command: schedule label plus status tags, if any. */
export function routineSubtitle(routine: RoutineResponse): string {
  const sched = scheduleLabel(routine.schedule_description, routine.schedule);
  const tags: string[] = [];
  if (!routine.enabled) {
    tags.push("DISABLED");
  } else if ((routine.skip_runs ?? 0) > 0) {
    tags.push("SNOOZED");
  } else if (!routine.agent_registered) {
    tags.push("AGENT MISSING");
  }
  if (routine.flag_count > 0) tags.push("FLAGS");
  return tags.length === 0 ? sched : `${sched} — ${tags.join(", ")}`;
}

/** Builds the full command list: pages first, then one entry per routine. */
export function buildCommands(routines: RoutineResponse[]): Command[] {
  const commands: Command[] = [
    {
      kind: "nav-overview",
      title: "Overview",
      subtitle: "Fleet summary & upcoming runs",
      keywords: "home dashboard kpi summary landing",
    },
    {
      kind: "nav-routines",
      title: "Routines",
      subtitle: "Manage agent-driven routines",
      keywords: "agents automation",
    },
    {
      kind: "nav-heatmap",
      title: "Heatmap",
      subtitle: "7-day x 24-hour fire-density grid",
      keywords: "schedule density grid busy collisions calendar",
    },
    {
      kind: "nav-reliability",
      title: "Reliability",
      subtitle: "Success rate, streaks & duration regressions",
      keywords: "flaky failing streak p50 p95 duration regression health",
    },
    {
      kind: "nav-machines",
      title: "Machines",
      subtitle: "Per-host fleet inventory",
      keywords: "fleet hosts inventory nodes servers",
    },
    {
      kind: "nav-settings",
      title: "Settings",
      subtitle: "Persistent agent prompt",
      keywords: "config preferences user prompt",
    },
    {
      kind: "action-refresh",
      title: "Refresh",
      subtitle: "Re-poll server health",
      keywords: "reload health status action",
    },
    {
      kind: "action-stop",
      title: "Stop Server",
      subtitle: "Shut the moadim server down",
      keywords: "shutdown halt kill quit action",
    },
    {
      kind: "action-toggle-theme",
      title: "Toggle Theme",
      subtitle: "Switch between dark and light mode",
      keywords: "theme light dark mode toggle appearance action",
    },
  ];
  for (const routine of routines) {
    commands.push({
      kind: "routine",
      title: routine.title,
      subtitle: routineSubtitle(routine),
      keywords: `${routine.id} ${routine.agent} ${routine.schedule} ${(routine.tags ?? []).join(" ")} routine`,
      routineId: routine.id,
    });
  }
  return commands;
}

/** Clamps `selected` to a valid row index for a result list of `len` rows. */
export function clampSelection(selected: number, len: number): number {
  return len === 0 ? 0 : Math.min(selected, len - 1);
}

/** Next selection index (↓): advances by one, never past the last row (no wrap). */
export function nextIndex(selected: number, len: number): number {
  return len === 0 ? 0 : Math.min(selected + 1, len - 1);
}

/** Previous selection index (↑): retreats by one, saturating at the first row (no wrap). */
export function prevIndex(selected: number): number {
  return Math.max(0, selected - 1);
}

/** Last selection index (End): the final row, or `0` when empty. */
export function lastIndex(len: number): number {
  return Math.max(0, len - 1);
}

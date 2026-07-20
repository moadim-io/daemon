// Ported 1:1 from ui/src/routines/filter_tests.rs, filter_distinct_tests.rs,
// filter_facet_codec_tests.rs, and filter_health_tests.rs.
import { describe, expect, it } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import {
  DUE_SOON_WINDOW_MS,
  distinctAgents,
  distinctMachines,
  distinctRepositories,
  distinctTags,
  filterRoutines,
  healthBadge,
  healthBadgeClass,
  healthPriority,
  isFilterActive,
  isRoutineSnoozed,
  lastFireAt,
  machineFacetValue,
  matchesFilter,
  namedFacetValue,
  parseMachineFacet,
  parseNamedFacet,
  parseStatusFacet,
  routineHealth,
  snoozeDetail,
  triggerButtonTitle,
  type NamedFacet,
  type RoutineFilter,
  type RoutineHealth,
} from "./filter";

function routine(
  id: string,
  title: string,
  agent: string,
  schedule: string,
  machines: string[],
  repos: string[],
  enabled: boolean,
  overrides: Partial<RoutineResponse> = {},
): RoutineResponse {
  return {
    id,
    title,
    agent,
    model: null,
    schedule,
    prompt: "",
    repositories: repos.map((r) => ({ repository: r, branch: null })),
    machines,
    enabled,
    source: "",
    created_at: 0,
    updated_at: 0,
    last_manual_trigger_at: null,
    last_scheduled_trigger_at: null,
    snoozed_until: null,
    skip_runs: null,
    power_saving: false,
    ttl_secs: null,
    tags: [],
    agent_registered: false,
    agent_command_available: false,
    is_running: false,
    file_path: "",
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

function defaultFilter(): RoutineFilter {
  return {
    query: "",
    status: "all",
    agent: { kind: "all" },
    machine: { kind: "any" },
    repository: { kind: "all" },
    tag: { kind: "all" },
  };
}

/** Fixed deterministic "now" for tests (2026-01-01 12:00:00 local). */
function now(): Date {
  return new Date(2026, 0, 1, 12, 0, 0);
}

const window = DUE_SOON_WINDOW_MS;

describe("filter", () => {
  // ── is_active ─────────────────────────────────────────────────────────────

  it("default filter is inactive", () => {
    expect(isFilterActive(defaultFilter())).toBe(false);
  });

  it("is_active detects each facet", () => {
    expect(isFilterActive({ ...defaultFilter(), query: "  x " })).toBe(true);
    // Whitespace-only query is not active.
    expect(isFilterActive({ ...defaultFilter(), query: "   " })).toBe(false);
    expect(isFilterActive({ ...defaultFilter(), status: "enabled" })).toBe(true);
    expect(isFilterActive({ ...defaultFilter(), status: "due" })).toBe(true);
    expect(isFilterActive({ ...defaultFilter(), agent: { kind: "named", value: "claude" } })).toBe(
      true,
    );
    expect(isFilterActive({ ...defaultFilter(), machine: { kind: "unassigned" } })).toBe(true);
    expect(
      isFilterActive({
        ...defaultFilter(),
        repository: { kind: "named", value: "github.com/org/repo" },
      }),
    ).toBe(true);
    expect(isFilterActive({ ...defaultFilter(), tag: { kind: "named", value: "nightly" } })).toBe(
      true,
    );
  });

  // ── Status facet matching ─────────────────────────────────────────────────

  it("status all matches regardless of enabled", () => {
    const f = defaultFilter();
    const on = routine("a", "t", "claude", "0 * * * *", ["m1"], [], true);
    const off = routine("b", "t", "claude", "0 * * * *", ["m1"], [], false);
    expect(matchesFilter(f, on, now(), window)).toBe(true);
    expect(matchesFilter(f, off, now(), window)).toBe(true);
  });

  it("status enabled and disabled partition", () => {
    const on = routine("a", "t", "claude", "0 * * * *", ["m1"], [], true);
    const off = routine("b", "t", "claude", "0 * * * *", ["m1"], [], false);
    const enabled: RoutineFilter = { ...defaultFilter(), status: "enabled" };
    const disabled: RoutineFilter = { ...defaultFilter(), status: "disabled" };
    expect(matchesFilter(enabled, on, now(), window)).toBe(true);
    expect(matchesFilter(enabled, off, now(), window)).toBe(false);
    expect(matchesFilter(disabled, off, now(), window)).toBe(true);
    expect(matchesFilter(disabled, on, now(), window)).toBe(false);
  });

  it("status dormant requires enabled and no machines", () => {
    const f: RoutineFilter = { ...defaultFilter(), status: "dormant" };
    const dormant = routine("a", "t", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, dormant, now(), window)).toBe(true);
    const active = routine("b", "t", "claude", "0 * * * *", ["m1"], [], true);
    expect(matchesFilter(f, active, now(), window)).toBe(false);
    const disabledNoMachine = routine("c", "t", "claude", "0 * * * *", [], [], false);
    expect(matchesFilter(f, disabledNoMachine, now(), window)).toBe(false);
  });

  it("status dormant matches a blank machine entry like routineHealth does", () => {
    // A machines list holding only whitespace is "no real machine assigned", same as an empty
    // list — matching routineHealth's definition so the status facet and the health badge/KPI
    // never disagree about the same routine.
    const f: RoutineFilter = { ...defaultFilter(), status: "dormant" };
    const blankMachine = routine("a", "t", "claude", "0 * * * *", ["   "], [], true);
    expect(matchesFilter(f, blankMachine, now(), window)).toBe(true);
  });

  it("status due soon matches enabled routines firing within window", () => {
    const f: RoutineFilter = { ...defaultFilter(), status: "due" };
    const imminent = routine("a", "t", "claude", "* * * * *", ["m1"], [], true);
    expect(matchesFilter(f, imminent, now(), window)).toBe(true);
    const disabled = routine("b", "t", "claude", "* * * * *", ["m1"], [], false);
    expect(matchesFilter(f, disabled, now(), window)).toBe(false);
    const boundary = routine("c", "t", "claude", "0 * * * *", ["m1"], [], true);
    expect(matchesFilter(f, boundary, now(), window)).toBe(true);
    const never = routine("d", "t", "claude", "", ["m1"], [], true);
    expect(matchesFilter(f, never, now(), window)).toBe(false);
  });

  it("status snoozed matches only snoozed routines", () => {
    const f: RoutineFilter = { ...defaultFilter(), status: "snoozed" };
    const snoozed = routine("a", "t", "claude", "0 * * * *", ["m1"], [], true, {
      snoozed_until: Math.floor(now().getTime() / 1000) + 3_600,
    });
    const active = routine("b", "t", "claude", "0 * * * *", ["m1"], [], true);
    const disabledSnoozed = routine("c", "t", "claude", "0 * * * *", ["m1"], [], false, {
      snoozed_until: Math.floor(now().getTime() / 1000) + 3_600,
    });
    expect(matchesFilter(f, snoozed, now(), window)).toBe(true);
    expect(matchesFilter(f, active, now(), window)).toBe(false);
    // Disabled+snoozed: snoozed filter does not check enabled state.
    expect(matchesFilter(f, disabledSnoozed, now(), window)).toBe(true);
  });

  it("status has flags matches only flagged routines", () => {
    const f: RoutineFilter = { ...defaultFilter(), status: "flagged" };
    const flagged = routine("a", "t", "claude", "0 * * * *", ["m1"], [], true, { flag_count: 2 });
    const clean = routine("b", "t", "claude", "0 * * * *", ["m1"], [], true);
    expect(matchesFilter(f, flagged, now(), window)).toBe(true);
    expect(matchesFilter(f, clean, now(), window)).toBe(false);
  });

  // ── Agent facet matching ──────────────────────────────────────────────────

  it("agent all matches any agent", () => {
    const f = defaultFilter();
    const c = routine("a", "t", "claude", "0 * * * *", ["m1"], [], true);
    const cx = routine("b", "t", "codex", "0 * * * *", ["m1"], [], true);
    expect(matchesFilter(f, c, now(), window)).toBe(true);
    expect(matchesFilter(f, cx, now(), window)).toBe(true);
  });

  it("agent named filters by exact agent", () => {
    const f: RoutineFilter = { ...defaultFilter(), agent: { kind: "named", value: "claude" } };
    const claude = routine("a", "t", "claude", "0 * * * *", ["m1"], [], true);
    const codex = routine("b", "t", "codex", "0 * * * *", ["m1"], [], true);
    expect(matchesFilter(f, claude, now(), window)).toBe(true);
    expect(matchesFilter(f, codex, now(), window)).toBe(false);
  });

  // ── Machine facet matching ────────────────────────────────────────────────

  it("machine any matches regardless of machines", () => {
    const f = defaultFilter();
    const withM = routine("a", "t", "claude", "0 * * * *", ["m1"], [], true);
    const without = routine("b", "t", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, withM, now(), window)).toBe(true);
    expect(matchesFilter(f, without, now(), window)).toBe(true);
  });

  it("machine unassigned matches only empty machines", () => {
    const f: RoutineFilter = { ...defaultFilter(), machine: { kind: "unassigned" } };
    const withM = routine("a", "t", "claude", "0 * * * *", ["m1"], [], true);
    const without = routine("b", "t", "claude", "0 * * * *", [], [], true);
    const blank = routine("c", "t", "claude", "0 * * * *", [""], [], true);
    expect(matchesFilter(f, withM, now(), window)).toBe(false);
    expect(matchesFilter(f, without, now(), window)).toBe(true);
    expect(matchesFilter(f, blank, now(), window)).toBe(true);
  });

  it("machine specific matches only that machine", () => {
    const f: RoutineFilter = { ...defaultFilter(), machine: { kind: "machine", value: "m1" } };
    const m1 = routine("a", "t", "claude", "0 * * * *", ["m1", "m2"], [], true);
    const m2Only = routine("b", "t", "claude", "0 * * * *", ["m2"], [], true);
    const none = routine("c", "t", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, m1, now(), window)).toBe(true);
    expect(matchesFilter(f, m2Only, now(), window)).toBe(false);
    expect(matchesFilter(f, none, now(), window)).toBe(false);
  });

  // ── Repository facet matching ─────────────────────────────────────────────

  it("repository all matches regardless of repositories", () => {
    const f = defaultFilter();
    const withR = routine("a", "t", "claude", "0 * * * *", [], ["repo-a"], true);
    const without = routine("b", "t", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, withR, now(), window)).toBe(true);
    expect(matchesFilter(f, without, now(), window)).toBe(true);
  });

  it("repository named matches only routines listing that repository", () => {
    const f: RoutineFilter = { ...defaultFilter(), repository: { kind: "named", value: "repo-a" } };
    const hit = routine("a", "t", "claude", "0 * * * *", [], ["repo-a", "repo-b"], true);
    const other = routine("b", "t", "claude", "0 * * * *", [], ["repo-b"], true);
    const none = routine("c", "t", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, hit, now(), window)).toBe(true);
    expect(matchesFilter(f, other, now(), window)).toBe(false);
    expect(matchesFilter(f, none, now(), window)).toBe(false);
  });

  // ── Tag facet matching ────────────────────────────────────────────────────

  it("tag all matches regardless of tags", () => {
    const f = defaultFilter();
    const withT = routine("a", "t", "claude", "0 * * * *", [], [], true, { tags: ["nightly"] });
    const without = routine("b", "t", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, withT, now(), window)).toBe(true);
    expect(matchesFilter(f, without, now(), window)).toBe(true);
  });

  it("tag named matches only routines carrying that tag", () => {
    const f: RoutineFilter = { ...defaultFilter(), tag: { kind: "named", value: "nightly" } };
    const hit = routine("a", "t", "claude", "0 * * * *", [], [], true, {
      tags: ["nightly", "prod"],
    });
    const other = routine("b", "t", "claude", "0 * * * *", [], [], true, { tags: ["prod"] });
    const none = routine("c", "t", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, hit, now(), window)).toBe(true);
    expect(matchesFilter(f, other, now(), window)).toBe(false);
    expect(matchesFilter(f, none, now(), window)).toBe(false);
  });

  // ── Free-text search ──────────────────────────────────────────────────────

  it("query matches title", () => {
    const f: RoutineFilter = { ...defaultFilter(), query: "deploy" };
    const hit = routine("a", "Deploy prod", "claude", "0 * * * *", [], [], true);
    const miss = routine("b", "Build images", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, hit, now(), window)).toBe(true);
    expect(matchesFilter(f, miss, now(), window)).toBe(false);
  });

  it("query matches agent", () => {
    const f: RoutineFilter = { ...defaultFilter(), query: "codex" };
    const hit = routine("a", "t", "codex", "0 * * * *", [], [], true);
    const miss = routine("b", "t", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, hit, now(), window)).toBe(true);
    expect(matchesFilter(f, miss, now(), window)).toBe(false);
  });

  it("query matches repository url", () => {
    const f: RoutineFilter = { ...defaultFilter(), query: "github.com/acme" };
    const hit = routine("a", "t", "claude", "0 * * * *", [], ["https://github.com/acme/backend"], true);
    const miss = routine("b", "t", "claude", "0 * * * *", [], ["https://github.com/other/foo"], true);
    expect(matchesFilter(f, hit, now(), window)).toBe(true);
    expect(matchesFilter(f, miss, now(), window)).toBe(false);
  });

  it("query matches tag", () => {
    const f: RoutineFilter = { ...defaultFilter(), query: "nightly" };
    const hit = routine("a", "t", "claude", "0 * * * *", [], [], true, { tags: ["nightly"] });
    const miss = routine("b", "t", "claude", "0 * * * *", [], [], true, { tags: ["prod"] });
    expect(matchesFilter(f, hit, now(), window)).toBe(true);
    expect(matchesFilter(f, miss, now(), window)).toBe(false);
  });

  it("query is case insensitive", () => {
    const f: RoutineFilter = { ...defaultFilter(), query: "DEPLOY" };
    const hit = routine("a", "deploy staging", "claude", "0 * * * *", [], [], true);
    expect(matchesFilter(f, hit, now(), window)).toBe(true);
  });

  it("empty query matches all", () => {
    const f: RoutineFilter = { ...defaultFilter(), query: "   " };
    const r = routine("a", "anything", "claude", "0 * * * *", ["m"], [], true);
    expect(matchesFilter(f, r, now(), window)).toBe(true);
  });

  // ── filter_routines helper ────────────────────────────────────────────────

  it("filter_routines returns only matching", () => {
    const routines = [
      routine("a", "alpha", "claude", "0 * * * *", ["m1"], [], true),
      routine("b", "beta", "codex", "0 * * * *", ["m1"], [], false),
      routine("c", "gamma", "claude", "0 * * * *", [], [], true),
    ];
    const f: RoutineFilter = { ...defaultFilter(), status: "enabled" };
    const got = filterRoutines(routines, f, now(), window);
    expect(got.length).toBe(2);
    expect(got.every((r) => r.enabled)).toBe(true);
  });

  it("filter_routines due soon returns imminent enabled only", () => {
    const routines = [
      routine("a", "frequent", "claude", "* * * * *", ["m1"], [], true),
      routine("b", "hourly", "claude", "0 * * * *", ["m1"], [], true),
      routine("c", "off", "claude", "* * * * *", ["m1"], [], false),
    ];
    const f: RoutineFilter = { ...defaultFilter(), status: "due" };
    const got = filterRoutines(routines, f, now(), window);
    expect(got.length).toBe(2);
    expect(got.every((r) => r.enabled)).toBe(true);
  });

  // ── distinct helpers (filter_distinct_tests.rs) ──────────────────────────

  it("distinct_agents returns sorted unique agents", () => {
    const routines = [
      routine("a", "t", "codex", "0 * * * *", [], [], true),
      routine("b", "t", "claude", "0 * * * *", [], [], true),
      routine("c", "t", "claude", "0 * * * *", [], [], true),
    ];
    expect(distinctAgents(routines)).toEqual(["claude", "codex"]);
  });

  it("distinct_machines returns sorted unique machines", () => {
    const routines = [
      routine("a", "t", "claude", "0 * * * *", ["m2", "m1"], [], true),
      routine("b", "t", "claude", "0 * * * *", ["m1", "m3"], [], true),
    ];
    expect(distinctMachines(routines)).toEqual(["m1", "m2", "m3"]);
  });

  it("distinct_machines omits blank machine entries", () => {
    const routines = [
      routine("a", "t", "claude", "0 * * * *", ["", "m1"], [], true),
      routine("b", "t", "claude", "0 * * * *", ["  "], [], true),
    ];
    expect(distinctMachines(routines)).toEqual(["m1"]);
  });

  it("distinct_repositories returns sorted unique repositories", () => {
    const routines = [
      routine("a", "t", "claude", "0 * * * *", [], ["repo-b", "repo-a"], true),
      routine("b", "t", "claude", "0 * * * *", [], ["repo-a", "repo-c"], true),
    ];
    expect(distinctRepositories(routines)).toEqual(["repo-a", "repo-b", "repo-c"]);
  });

  it("distinct_tags returns sorted unique tags", () => {
    const routines = [
      routine("a", "t", "claude", "0 * * * *", [], [], true, { tags: ["nightly", "beta"] }),
      routine("b", "t", "claude", "0 * * * *", [], [], true, { tags: ["beta", "prod"] }),
    ];
    expect(distinctTags(routines)).toEqual(["beta", "nightly", "prod"]);
  });

  // ── Facet codecs (filter_facet_codec_tests.rs) ───────────────────────────

  it("status facet roundtrips and defaults to all", () => {
    const all: readonly string[] = [
      "all",
      "enabled",
      "disabled",
      "dormant",
      "due",
      "snoozed",
      "flagged",
      "agent-unreg",
    ];
    for (const s of all) expect(parseStatusFacet(s)).toBe(s);
    expect(parseStatusFacet("nonsense")).toBe("all");
  });

  it("agent facet roundtrips and defaults to all", () => {
    const all: NamedFacet = { kind: "all" };
    const namedF: NamedFacet = { kind: "named", value: "claude" };
    expect(parseNamedFacet(namedFacetValue(all))).toEqual(all);
    expect(parseNamedFacet(namedFacetValue(namedF))).toEqual(namedF);
  });

  it("agent facet decodes a plain name as named", () => {
    expect(parseNamedFacet("codex")).toEqual({ kind: "named", value: "codex" });
  });

  it("repository facet roundtrips and defaults to all", () => {
    const all: NamedFacet = { kind: "all" };
    const namedF: NamedFacet = { kind: "named", value: "github.com/org/repo" };
    expect(parseNamedFacet(namedFacetValue(all))).toEqual(all);
    expect(parseNamedFacet(namedFacetValue(namedF))).toEqual(namedF);
  });

  it("repository facet decodes a plain url as named", () => {
    expect(parseNamedFacet("github.com/org/repo")).toEqual({
      kind: "named",
      value: "github.com/org/repo",
    });
  });

  it("tag facet roundtrips and defaults to all", () => {
    const all: NamedFacet = { kind: "all" };
    const namedF: NamedFacet = { kind: "named", value: "nightly" };
    expect(parseNamedFacet(namedFacetValue(all))).toEqual(all);
    expect(parseNamedFacet(namedFacetValue(namedF))).toEqual(namedF);
  });

  it("tag facet decodes a plain value as named", () => {
    expect(parseNamedFacet("nightly")).toEqual({ kind: "named", value: "nightly" });
  });

  it("machine facet roundtrips through select value", () => {
    const any = { kind: "any" as const };
    const unassigned = { kind: "unassigned" as const };
    const specific = { kind: "machine" as const, value: "alpha" };
    expect(parseMachineFacet(machineFacetValue(any))).toEqual(any);
    expect(parseMachineFacet(machineFacetValue(unassigned))).toEqual(unassigned);
    expect(parseMachineFacet(machineFacetValue(specific))).toEqual(specific);
  });

  it("machine facet decodes a plain id as specific", () => {
    expect(parseMachineFacet("worker-1")).toEqual({ kind: "machine", value: "worker-1" });
  });

  // ── last_fire_at (filter_health_tests.rs) ────────────────────────────────

  function routineWithTriggers(
    lastManual: number | null,
    lastScheduled: number | null,
  ): RoutineResponse {
    return routine("id", "My Routine", "claude", "0 * * * *", [], [], true, {
      last_manual_trigger_at: lastManual,
      last_scheduled_trigger_at: lastScheduled,
    });
  }

  it("last_fire_at none when never triggered", () => {
    expect(lastFireAt(routineWithTriggers(null, null))).toBeUndefined();
  });

  it("last_fire_at manual only", () => {
    expect(lastFireAt(routineWithTriggers(100, null))).toBe(100);
  });

  it("last_fire_at scheduled only", () => {
    expect(lastFireAt(routineWithTriggers(null, 200))).toBe(200);
  });

  it("last_fire_at returns max when manual is later", () => {
    expect(lastFireAt(routineWithTriggers(300, 100))).toBe(300);
  });

  it("last_fire_at returns max when scheduled is later", () => {
    expect(lastFireAt(routineWithTriggers(100, 300))).toBe(300);
  });

  it("last_fire_at equal timestamps returns that value", () => {
    expect(lastFireAt(routineWithTriggers(500, 500))).toBe(500);
  });

  // ── routine_health ────────────────────────────────────────────────────────

  it("health disabled routine is disabled", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], false);
    expect(routineHealth(r, now())).toBe("disabled");
  });

  it("health power saving routine is power saving", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      power_saving: true,
    });
    expect(routineHealth(r, now())).toBe("power-saving");
  });

  it("health disabled outranks power saving", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], false, {
      power_saving: true,
    });
    expect(routineHealth(r, now())).toBe("disabled");
  });

  it("trigger_button_title names the pause reason", () => {
    const disabled = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], false);
    expect(triggerButtonTitle(disabled)).toBe("Routine is disabled");

    const powerSaving = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      power_saving: true,
    });
    expect(triggerButtonTitle(powerSaving)).toBe("Routine is in power-saving mode");

    const healthy = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true);
    expect(triggerButtonTitle(healthy)).toBe("Run now");
  });

  it("health enabled no machines is dormant", () => {
    const r = routine("a", "A", "claude", "0 * * * *", [], [], true, { agent_registered: true });
    expect(routineHealth(r, now())).toBe("dormant");
  });

  it("health enabled blank machine entry is dormant", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["   "], [], true, {
      agent_registered: true,
    });
    expect(routineHealth(r, now())).toBe("dormant");
  });

  it("health dead schedule is dead", () => {
    const r = routine("a", "A", "claude", "not-a-valid-cron", ["machine1"], [], true, {
      agent_registered: true,
    });
    expect(routineHealth(r, now())).toBe("dead-schedule");
  });

  it("health missing agent is agent missing", () => {
    // agent_registered defaults to false in routine()
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true);
    expect(routineHealth(r, now())).toBe("agent-missing");
  });

  it("health fully configured is healthy", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      agent_registered: true,
    });
    expect(routineHealth(r, now())).toBe("healthy");
  });

  // ─── snooze_detail ────────────────────────────────────────────────────────

  it("snooze_detail empty when not snoozed", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["m"], [], true);
    expect(snoozeDetail(r, now())).toBe("");
  });

  it("snooze_detail shows minutes left for short snooze", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["m"], [], true, {
      snoozed_until: Math.floor(now().getTime() / 1000) + 45 * 60,
    });
    expect(snoozeDetail(r, now())).toBe("45m left");
  });

  it("snooze_detail shows hours left", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["m"], [], true, {
      snoozed_until: Math.floor(now().getTime() / 1000) + 3 * 3_600,
    });
    expect(snoozeDetail(r, now())).toBe("3h left");
  });

  it("snooze_detail shows days left", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["m"], [], true, {
      snoozed_until: Math.floor(now().getTime() / 1000) + 2 * 86_400,
    });
    expect(snoozeDetail(r, now())).toBe("2d left");
  });

  it("snooze_detail shows skip runs", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["m"], [], true, { skip_runs: 5 });
    expect(snoozeDetail(r, now())).toBe("5 runs skipped");
  });

  it("snooze_detail skip runs singular", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["m"], [], true, { skip_runs: 1 });
    expect(snoozeDetail(r, now())).toBe("1 run skipped");
  });

  it("snooze_detail empty when deadline past", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["m"], [], true, {
      snoozed_until: Math.floor(now().getTime() / 1000) - 3_600,
    });
    expect(snoozeDetail(r, now())).toBe("");
  });

  it("is_routine_snoozed true when deadline in future", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      snoozed_until: Math.floor(now().getTime() / 1000) + 3_600,
    });
    expect(isRoutineSnoozed(r, now())).toBe(true);
  });

  it("is_routine_snoozed false when deadline past", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      snoozed_until: Math.floor(now().getTime() / 1000) - 3_600,
    });
    expect(isRoutineSnoozed(r, now())).toBe(false);
  });

  it("is_routine_snoozed true when skip_runs nonzero", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, { skip_runs: 3 });
    expect(isRoutineSnoozed(r, now())).toBe(true);
  });

  it("is_routine_snoozed false when skip_runs zero", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, { skip_runs: 0 });
    expect(isRoutineSnoozed(r, now())).toBe(false);
  });

  it("health snoozed until future is snoozed", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      agent_registered: true,
      snoozed_until: Math.floor(now().getTime() / 1000) + 3_600,
    });
    expect(routineHealth(r, now())).toBe("snoozed");
  });

  it("health snoozed until past is healthy", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      agent_registered: true,
      snoozed_until: Math.floor(now().getTime() / 1000) - 3_600,
    });
    expect(routineHealth(r, now())).toBe("healthy");
  });

  it("health skip_runs above zero is snoozed", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      agent_registered: true,
      skip_runs: 2,
    });
    expect(routineHealth(r, now())).toBe("snoozed");
  });

  it("health skip_runs zero is healthy", () => {
    const r = routine("a", "A", "claude", "0 * * * *", ["machine1"], [], true, {
      agent_registered: true,
      skip_runs: 0,
    });
    expect(routineHealth(r, now())).toBe("healthy");
  });

  it("health priority order dormant most urgent", () => {
    expect(healthPriority("dormant")).toBeLessThan(healthPriority("dead-schedule"));
    expect(healthPriority("dead-schedule")).toBeLessThan(healthPriority("agent-missing"));
    expect(healthPriority("agent-missing")).toBeLessThan(healthPriority("disabled"));
    expect(healthPriority("disabled")).toBeLessThan(healthPriority("power-saving"));
    expect(healthPriority("power-saving")).toBeLessThan(healthPriority("snoozed"));
    expect(healthPriority("snoozed")).toBeLessThan(healthPriority("healthy"));
  });

  // `healthBadge`/`healthBadgeClass` were the only exported `filter.ts` functions with no test
  // (mirrors ui/src/routines/filter_health_tests.rs's `health_badge_and_badge_class_cover_all_variants`,
  // added alongside this test) — assert the exact rendered strings for every variant, and that
  // both stay unique, so a copy-paste badge/class collision is caught here instead of silently in
  // the UI.
  it("health badge and badge class cover all variants", () => {
    const cases: Array<[RoutineHealth, string, string]> = [
      ["dormant", "DORMANT", "health-badge dormant"],
      ["dead-schedule", "DEAD SCHEDULE", "health-badge dead"],
      ["agent-missing", "AGENT MISSING", "health-badge agent-missing"],
      ["disabled", "DISABLED", "health-badge disabled"],
      ["power-saving", "POWER SAVING", "health-badge power-saving"],
      ["snoozed", "SNOOZED", "health-badge snoozed"],
      ["healthy", "HEALTHY", "health-badge healthy"],
    ];
    for (const [health, badge, badgeClass] of cases) {
      expect(healthBadge(health)).toBe(badge);
      expect(healthBadgeClass(health)).toBe(badgeClass);
    }
    const badges = cases.map(([health]) => healthBadge(health));
    const classes = cases.map(([health]) => healthBadgeClass(health));
    expect(new Set(badges).size).toBe(badges.length);
    expect(new Set(classes).size).toBe(classes.length);
  });
});

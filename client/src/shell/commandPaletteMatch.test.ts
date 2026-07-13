import { describe, expect, it } from "vitest";
import type { RoutineResponse } from "../api/hooks";
import {
  badgeFor,
  buildCommands,
  clampSelection,
  fuzzyScore,
  lastIndex,
  nextIndex,
  prevIndex,
  rank,
  routeFor,
  routineSubtitle,
  scheduleLabel,
  type Command,
} from "./commandPaletteMatch";

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "r1",
    title: "T",
    agent: "claude",
    schedule: "*/5 * * * *",
    model: null,
    prompt: "",
    repositories: [],
    machines: [],
    enabled: true,
    source: "managed",
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
    timezone: null,
    goal: null,
    flag_count: 0,
    next_run_at: null,
    ...overrides,
  };
}

function cmd(kind: Command["kind"], title: string, keywords: string): Command {
  return { kind, title, subtitle: "", keywords };
}

describe("fuzzyScore", () => {
  it("matches everything with a neutral score for an empty query", () => {
    expect(fuzzyScore("anything", "")).toBe(0);
    expect(fuzzyScore("anything", "   ")).toBe(0);
  });

  it("does not match a non-subsequence", () => {
    expect(fuzzyScore("abc", "xyz")).toBeUndefined();
    expect(fuzzyScore("abc", "cab")).toBeUndefined();
  });

  it("ranks a leading hit above a mid-string hit", () => {
    const lead = fuzzyScore("overview", "o")!;
    const mid = fuzzyScore("retro", "o")!;
    expect(lead).toBeGreaterThan(mid);
  });

  it("rewards a word-boundary hit over an interior hit", () => {
    const boundary = fuzzyScore("nightly jobs", "j")!;
    const interior = fuzzyScore("major", "j")!;
    expect(boundary).toBeGreaterThan(interior);
  });

  it("rewards a consecutive run over a scattered match", () => {
    const consecutive = fuzzyScore("zzabczz", "abc")!;
    const scattered = fuzzyScore("axbxc", "abc")!;
    expect(consecutive).toBeGreaterThan(scattered);
  });

  it("penalizes longer text", () => {
    const short = fuzzyScore("ab", "a")!;
    const long = fuzzyScore("a".repeat(40), "a")!;
    expect(short).toBeGreaterThan(long);
  });

  it("is case-insensitive", () => {
    expect(fuzzyScore("Overview", "OVERVIEW")).toBeDefined();
  });
});

describe("rank", () => {
  it("keeps natural order for an empty query", () => {
    const commands = [cmd("nav-overview", "Overview", ""), cmd("routine", "backup", "")];
    expect(rank(commands, "")).toEqual([0, 1]);
  });

  it("ranks a title match above a keyword-only match", () => {
    const commands = [
      cmd("routine", "nightly", "backup database"),
      cmd("routine", "backup", "misc"),
    ];
    const order = rank(commands, "backup");
    expect(order[0]).toBe(1);
    expect(order).toHaveLength(2);
  });

  it("still surfaces a keyword-only match", () => {
    const commands = [cmd("routine", "alpha", "zeta")];
    expect(rank(commands, "zeta")).toEqual([0]);
  });

  it("matches when only the title hits", () => {
    const commands = [cmd("routine", "deploy", "xxxxx")];
    expect(rank(commands, "deploy")).toEqual([0]);
  });

  it("drops non-matching commands", () => {
    const commands = [cmd("routine", "alpha", "one"), cmd("routine", "beta", "two")];
    expect(rank(commands, "zzz")).toEqual([]);
  });
});

describe("buildCommands", () => {
  it("includes routine tags in keywords", () => {
    const r = routine({ title: "Report", tags: ["security", "weekly"], agent_registered: true });
    const commands = buildCommands([r]);
    const kw = commands[commands.length - 1]!.keywords;
    expect(kw).toContain("security");
    expect(kw).toContain("weekly");
  });

  it("lists pages then actions then routines", () => {
    const commands = buildCommands([routine({ title: "Nightly Audit", schedule: "0 0 * * *" })]);
    expect(commands).toHaveLength(8); // 4 nav + 3 action + 1 routine
    expect(commands.map((c) => c.kind)).toEqual([
      "nav-overview",
      "nav-routines",
      "nav-heatmap",
      "nav-settings",
      "action-refresh",
      "action-stop",
      "action-toggle-theme",
      "routine",
    ]);
    expect(commands[6]!.keywords).toContain("theme");
    expect(commands[7]!.title).toBe("Nightly Audit");
    expect(commands[7]!.subtitle).toBe("0 0 * * * — AGENT MISSING");
    expect(commands[7]!.keywords).toContain("claude");
  });
});

describe("scheduleLabel", () => {
  it("prefers human, then raw, then a dash", () => {
    expect(scheduleLabel("At noon", "0 12 * * *")).toBe("At noon");
    expect(scheduleLabel("", "0 12 * * *")).toBe("0 12 * * *");
    expect(scheduleLabel(null, "0 12 * * *")).toBe("0 12 * * *");
    expect(scheduleLabel(null, "   ")).toBe("—");
  });
});

describe("routineSubtitle", () => {
  it("shows only the schedule when healthy", () => {
    const r = routine({
      enabled: true,
      agent_registered: true,
      flag_count: 0,
      schedule_description: "Every 5 minutes",
    });
    expect(routineSubtitle(r)).toBe("Every 5 minutes");
  });

  it("appends DISABLED", () => {
    const r = routine({ enabled: false, agent_registered: true });
    expect(routineSubtitle(r).endsWith("— DISABLED")).toBe(true);
  });

  it("appends SNOOZED via skip_runs", () => {
    const r = routine({ enabled: true, agent_registered: true, skip_runs: 2 });
    expect(routineSubtitle(r).endsWith("— SNOOZED")).toBe(true);
  });

  it("appends AGENT MISSING", () => {
    const r = routine({ enabled: true, agent_registered: false });
    expect(routineSubtitle(r).endsWith("— AGENT MISSING")).toBe(true);
  });

  it("appends FLAGS even when otherwise healthy", () => {
    const r = routine({ enabled: true, agent_registered: true, flag_count: 3 });
    expect(routineSubtitle(r).endsWith("— FLAGS")).toBe(true);
  });

  it("shows both DISABLED and FLAGS", () => {
    const r = routine({ enabled: false, agent_registered: true, flag_count: 2 });
    const s = routineSubtitle(r);
    expect(s).toContain("DISABLED");
    expect(s).toContain("FLAGS");
  });
});

describe("routeFor / badgeFor", () => {
  it("maps every kind to its route", () => {
    expect(routeFor("nav-overview")).toBe("home");
    expect(routeFor("nav-routines")).toBe("routines");
    expect(routeFor("nav-heatmap")).toBe("heatmap");
    expect(routeFor("nav-settings")).toBe("settings");
    expect(routeFor("routine")).toBe("routines");
    expect(routeFor("action-refresh")).toBeUndefined();
    expect(routeFor("action-stop")).toBeUndefined();
    expect(routeFor("action-toggle-theme")).toBeUndefined();
  });

  it("maps every kind to its badge", () => {
    expect(badgeFor("nav-overview")).toBe("GO");
    expect(badgeFor("nav-routines")).toBe("GO");
    expect(badgeFor("nav-heatmap")).toBe("GO");
    expect(badgeFor("nav-settings")).toBe("GO");
    expect(badgeFor("routine")).toBe("ROUTINE");
    expect(badgeFor("action-refresh")).toBe("ACTION");
    expect(badgeFor("action-stop")).toBe("ACTION");
    expect(badgeFor("action-toggle-theme")).toBe("ACTION");
  });
});

describe("selection-index helpers", () => {
  it("clampSelection handles empty and overflow", () => {
    expect(clampSelection(5, 0)).toBe(0);
    expect(clampSelection(2, 4)).toBe(2);
    expect(clampSelection(9, 4)).toBe(3);
  });

  it("nextIndex advances without wrapping", () => {
    expect(nextIndex(0, 0)).toBe(0);
    expect(nextIndex(0, 3)).toBe(1);
    expect(nextIndex(2, 3)).toBe(2);
  });

  it("prevIndex saturates at zero", () => {
    expect(prevIndex(0)).toBe(0);
    expect(prevIndex(3)).toBe(2);
  });

  it("lastIndex is the final row or zero", () => {
    expect(lastIndex(0)).toBe(0);
    expect(lastIndex(5)).toBe(4);
  });
});

import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import {
  computeRepoStats,
  matchesRepoQuery,
  repoShortName,
  repoSuccessRate,
  routinesWithoutRepo,
  type RepoStats,
} from "./reposStats";

function routine(id: string, overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id,
    title: id,
    agent: "claude",
    model: null,
    schedule: "* * * * *",
    prompt: "",
    repositories: [],
    machines: ["box-1"],
    enabled: true,
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
    agent_registered: true,
    agent_command_available: true,
    agent_setup_available: true,
    is_running: false,
    file_path: "",
    folder: null,
    slug: "routine",
    rel_path: "routine",
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

function run(routineId: string, startedAt: number, overrides: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: routineId,
    workbench: `wb-${startedAt}`,
    started_at: startedAt,
    started_at_local: `t${startedAt}`,
    status: "success",
    exit_code: 0,
    finished_at: startedAt + 10,
    finished_at_local: "",
    ...overrides,
  };
}

const API = "https://github.com/acme/api.git";
const WEB = "git@github.com:acme/web.git";

describe("repoShortName", () => {
  it("keeps the last two segments of an https URL", () => {
    expect(repoShortName(API)).toBe("acme/api");
  });

  it("keeps the last two segments of an scp-style ssh URL", () => {
    expect(repoShortName(WEB)).toBe("acme/web");
  });

  it("ignores surrounding whitespace and trailing slashes", () => {
    expect(repoShortName("  https://github.com/acme/api/  ")).toBe("acme/api");
  });

  it("returns a bare name unchanged", () => {
    expect(repoShortName("local-repo")).toBe("local-repo");
  });
});

describe("computeRepoStats", () => {
  it("pivots routines by repository and rolls up their state", () => {
    const stats = computeRepoStats(
      [
        routine("r1", {
          title: "build",
          agent: "codex",
          is_running: true,
          repositories: [{ repository: API, branch: " main " }, { repository: API, branch: "main" }],
        }),
        routine("r2", {
          title: "lint",
          enabled: false,
          repositories: [{ repository: API, branch: "  " }, { repository: WEB, auto_pull: false }],
        }),
        routine("r3", { title: "docs", repositories: [{ repository: API, branch: "dev", auto_pull: false }] }),
      ],
      [
        run("other", 999),
        run("r1", 300, { status: "running", finished_at: null }),
        run("r3", 200, { status: "failed", finished_at: 210 }),
        run("r1", 100, { finished_at: 130 }),
        run("r2", 50, { finished_at: 40 }),
      ],
    );
    expect(stats.map((r) => r.url)).toEqual([API, WEB]);
    const api = stats[0]!;
    expect(api).toMatchObject({
      name: "acme/api",
      total: 3,
      enabled: 2,
      runningNow: 1,
      branches: ["default", "dev", "main"],
      agents: ["claude", "codex"],
      autoPullOff: 1,
      failing: 1,
      successCount: 2,
      finishedCount: 3,
      p50Secs: 10,
      lastRun: { label: "t300", status: "running" },
    });
    expect(api.routines.map((r) => r.title)).toEqual(["build", "docs", "lint"]);
    expect(repoSuccessRate(api)).toBeCloseTo(2 / 3);
    expect(stats[1]).toMatchObject({ total: 1, autoPullOff: 1, failing: 0, p50Secs: null });
  });

  it("uses the newest finished run per routine for failing, skipping running runs", () => {
    const stats = computeRepoStats(
      [routine("a", { repositories: [{ repository: API }] }), routine("b", { repositories: [{ repository: API }] })],
      [
        run("a", 400, { status: "failed" }),
        run("a", 300, { status: "success" }),
        run("a", 200, { status: "running", finished_at: null }),
        run("b", 100, { status: "success" }),
        run("b", 50, { status: "failed" }),
      ],
    );
    expect(stats[0]).toMatchObject({ failing: 1, finishedCount: 4, successCount: 2 });
  });

  it("ignores finished runs with a missing or backwards finish time for p50", () => {
    const stats = computeRepoStats(
      [routine("a", { repositories: [{ repository: API }] })],
      [
        run("a", 100, { finished_at: 105 }),
        run("a", 200, { finished_at: null }),
        run("a", 300, { finished_at: 250 }),
        run("a", 400, { finished_at: 430 }),
      ],
    );
    expect(stats[0]).toMatchObject({ finishedCount: 4, p50Secs: 5 });
  });

  it("reports no last run when no assigned routine ran", () => {
    const stats = computeRepoStats([routine("a", { repositories: [{ repository: API }] })], [run("zz", 1)]);
    expect(stats[0]).toMatchObject({ lastRun: null, finishedCount: 0, p50Secs: null });
    expect(repoSuccessRate(stats[0]!)).toBeNull();
  });

  it("ranks failing first, then busiest, then by name", () => {
    const stats = computeRepoStats(
      [
        routine("a", { repositories: [{ repository: "https://h/zeta/one" }] }),
        routine("b", { repositories: [{ repository: "https://h/alpha/two" }] }),
        routine("c", { repositories: [{ repository: "https://h/beta/three" }] }),
        routine("d", { repositories: [{ repository: "https://h/beta/three" }] }),
        routine("e", { repositories: [{ repository: "https://h/gamma/four" }] }),
      ],
      [run("e", 1, { status: "failed" })],
    );
    expect(stats.map((r) => r.name)).toEqual(["gamma/four", "beta/three", "alpha/two", "zeta/one"]);
  });

  it("returns nothing when no routine declares a repository", () => {
    expect(computeRepoStats([routine("a")], [])).toEqual([]);
    expect(computeRepoStats([], [])).toEqual([]);
  });
});

describe("repoSuccessRate", () => {
  it("is null without finished runs and a ratio otherwise", () => {
    const base = computeRepoStats([routine("a", { repositories: [{ repository: API }] })], [])[0] as RepoStats;
    expect(repoSuccessRate(base)).toBeNull();
    expect(repoSuccessRate({ ...base, successCount: 3, finishedCount: 4 })).toBe(0.75);
  });
});

describe("routinesWithoutRepo", () => {
  it("counts routines with a missing or empty repositories list", () => {
    expect(
      routinesWithoutRepo([
        routine("a"),
        routine("b", { repositories: [] }),
        routine("c", { repositories: [{ repository: API }] }),
        routine("d", { repositories: undefined }),
      ]),
    ).toBe(3);
  });
});

describe("matchesRepoQuery", () => {
  const [r] = computeRepoStats([routine("a", { repositories: [{ repository: API }] })], []);
  const repo = r!;

  it("matches the short name or the URL, case-insensitively and trimmed", () => {
    expect(matchesRepoQuery(repo, "  ACME/API ")).toBe(true);
    expect(matchesRepoQuery(repo, "github.com/acme")).toBe(true);
    expect(matchesRepoQuery(repo, "web")).toBe(false);
  });

  it("matches everything for an empty query", () => {
    expect(matchesRepoQuery(repo, "   ")).toBe(true);
  });
});

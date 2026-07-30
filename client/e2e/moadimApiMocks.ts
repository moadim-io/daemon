import type { Page, Route } from "@playwright/test";

export const NOW_MS = Date.UTC(2026, 6, 31, 9, 0, 0);
const NOW_SECS = Math.floor(NOW_MS / 1000);

const routines = [
  routine({
    id: "routine-daily-digest",
    title: "Daily release digest",
    goal: "Summarize release-critical changes and blockers for M1.",
    schedule: "0 9 * * 1-5",
    tags: ["digest", "release"],
    machines: ["m1"],
    nextRunAt: NOW_SECS + 3_600,
    status: { running: false, flags: 1 },
  }),
  routine({
    id: "routine-screen-tests",
    title: "Moadim screen tests",
    goal: "Regenerate UI screenshot evidence before PR review.",
    schedule: "*/30 * * * *",
    tags: ["qa", "ui"],
    machines: ["m1", "mini-lab"],
    nextRunAt: NOW_SECS + 1_800,
    status: { running: true, flags: 0 },
  }),
  routine({
    id: "routine-learning-loop",
    title: "Skill learning loop",
    goal: "Learn from recent fixes and update reusable skills.",
    schedule: "0 */4 * * *",
    tags: ["learning"],
    machines: ["mini-lab"],
    nextRunAt: NOW_SECS + 7_200,
    status: { running: false, powerSaving: true, flags: 2 },
  }),
];

const runs = [
  run({ routineId: "routine-screen-tests", title: "Moadim screen tests", status: "running", startedAgo: 420 }),
  run({ routineId: "routine-daily-digest", title: "Daily release digest", status: "success", startedAgo: 7_200, duration: 91 }),
  run({ routineId: "routine-learning-loop", title: "Skill learning loop", status: "failed", startedAgo: 14_400, duration: 262, exitCode: 1 }),
  run({ routineId: "routine-daily-digest", title: "Daily release digest", status: "success", startedAgo: 93_600, duration: 76 }),
  run({ routineId: "routine-screen-tests", title: "Moadim screen tests", status: "success", startedAgo: 97_200, duration: 118 }),
  run({ routineId: "routine-learning-loop", title: "Skill learning loop", status: "success", startedAgo: 104_400, duration: 205 }),
];

export async function installApiMocks(page: Page) {
  await page.route("**/api/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname.replace("/api/v1", "");

    if (request.method() !== "GET") {
      await route.fulfill({ status: 204, body: "" });
      return;
    }

    if (path === "/health") {
      await json(route, health());
      return;
    }
    if (path === "/machine") {
      await json(route, { name: "m1" });
      return;
    }
    if (path === "/machines") {
      await json(route, ["m1", "mini-lab"]);
      return;
    }
    if (path === "/routines") {
      await json(route, routines);
      return;
    }
    if (path === "/routines/runs") {
      await json(route, runs);
      return;
    }
    if (path === "/routines/lock") {
      await json(route, { local: false, locked: false, shared: false });
      return;
    }
    if (path === "/config/user-prompt") {
      await route.fulfill({ status: 200, contentType: "text/plain", body: "Always report blockers with evidence." });
      return;
    }

    await route.fulfill({ status: 404, contentType: "application/json", body: JSON.stringify({ error: path }) });
  });
}

async function json(route: Route, body: unknown) {
  await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) });
}

function health() {
  return {
    build_date: "2026-07-31",
    dependencies: { python3: true, tmux: true },
    git_sha: "e2e1234",
    machine: "m1",
    running: true,
    server_exe_dir: "/opt/moadim/bin",
    server_root: "/Users/ofek/.config/moadim",
    status: "ok",
    uptime_secs: 12_345,
    version: "1.6.0-e2e",
  };
}

type RoutineSeed = {
  id: string;
  title: string;
  goal: string;
  schedule: string;
  tags: string[];
  machines: string[];
  nextRunAt: number;
  status: { running: boolean; powerSaving?: boolean; flags: number };
};

function routine(seed: RoutineSeed) {
  return {
    id: seed.id,
    title: seed.title,
    goal: seed.goal,
    schedule: seed.schedule,
    tags: seed.tags,
    machines: seed.machines,
    agent: "hermes",
    enabled: true,
    source: "managed",
    prompt: "Run the routine with evidence and concise reporting.",
    repositories: [{ repository: "https://github.com/moadim-io/daemon", branch: "main" }],
    env_keys: ["HERMES_PROFILE"],
    file_path: `/Users/ofek/.config/moadim/routines/${seed.id}/routine.toml`,
    flag_count: seed.status.flags,
    is_running: seed.status.running,
    power_saving: seed.status.powerSaving ?? false,
    agent_registered: true,
    agent_command_available: true,
    agent_setup_available: true,
    created_at: NOW_SECS - 864_000,
    updated_at: NOW_SECS - 3_600,
    next_run_at: seed.nextRunAt,
    schedule_description: "Every weekday at 09:00 Asia/Jerusalem",
    timezone: "Asia/Jerusalem",
    ttl_secs: 86_400,
    max_runtime_secs: 3_600,
    last_manual_trigger_at: NOW_SECS - 7_200,
    last_scheduled_trigger_at: NOW_SECS - 86_400,
  };
}

type RunSeed = {
  routineId: string;
  title: string;
  status: "running" | "success" | "failed" | "unknown";
  startedAgo: number;
  duration?: number;
  exitCode?: number;
};

function run(seed: RunSeed) {
  const started = NOW_SECS - seed.startedAgo;
  const finished = seed.duration === undefined ? null : started + seed.duration;
  return {
    routine_id: seed.routineId,
    routine_title: seed.title,
    status: seed.status,
    started_at: started,
    started_at_local: localTime(started),
    finished_at: finished,
    finished_at_local: finished === null ? null : localTime(finished),
    exit_code: seed.exitCode ?? (seed.status === "failed" ? 1 : seed.status === "success" ? 0 : null),
    workbench: `${seed.routineId}-${started}`,
  };
}

function localTime(unixSecs: number) {
  return new Date(unixSecs * 1000).toLocaleString("en-US", { timeZone: "Asia/Jerusalem" });
}

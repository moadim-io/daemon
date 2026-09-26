import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { computeAdherence, GRACE_SECS, matchFires, pastFires } from "./adherenceMath";
import { ScheduleAdherence } from "./ScheduleAdherence";

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "a",
    schedule: "0 * * * *",
    title: "Alpha",
    agent: "a",
    enabled: true,
    source: "",
    created_at: 0,
    updated_at: 0,
    agent_registered: false,
    agent_command_available: false,
    agent_setup_available: false,
    is_running: false,
    file_path: "",
    folder: null,
    slug: "routine",
    rel_path: "routine",
    flag_count: 0,
    env_keys: [],
    machines: ["m1"],
    ...overrides,
  };
}

function run(routine_id: string, started_at: number): FleetRunSummary {
  return {
    routine_id,
    routine_title: routine_id,
    workbench: `wb-${started_at}`,
    started_at,
    started_at_local: "",
    finished_at: started_at + 60,
    finished_at_local: "",
    status: "success",
    exit_code: 0,
  };
}

// 2026-01-02T00:00:00Z — hourly fires land on whole hours in any whole-hour-offset timezone.
const NOW = Date.UTC(2026, 0, 2) / 1000;
const H = 3_600;

describe("pastFires", () => {
  it("lists fires inside [from, to] merged and de-duplicated across schedules", () => {
    const r = routine({ schedules: ["0 * * * *", "0 */2 * * *"] });
    expect(pastFires(r, NOW - 3 * H, NOW)).toEqual([NOW - 3 * H, NOW - 2 * H, NOW - H, NOW]);
  });

  it("is empty for an unparseable schedule", () => {
    expect(pastFires(routine({ schedule: "nope" }), NOW - H, NOW)).toEqual([]);
  });
});

describe("matchFires", () => {
  it("pairs each fire with one run inside the grace window and records misses", () => {
    const { lags, missedAt } = matchFires([100, 1_000, 5_000], [1_030, 90, 1_040, 5_000 + GRACE_SECS + 1]);
    expect(lags).toEqual([0, 30]);
    expect(missedAt).toEqual([5_000]);
  });
});

describe("computeAdherence", () => {
  it("audits only enabled routines on this machine and ranks the most-missed first", () => {
    const routines = [
      routine({ id: "a", title: "Alpha" }),
      routine({ id: "b", title: "Beta", created_at: NOW - 4 * H - 1 }),
      routine({ id: "off", enabled: false }),
      routine({ id: "other", machines: ["m2"] }),
      routine({ id: "dormant", machines: [" "] }),
    ];
    // Alpha ran every hour except 3h ago, 2 minutes late; Beta never ran.
    const runs = Array.from({ length: 24 }, (_, i) => NOW - (i + 1) * H + 120)
      .filter((t) => t !== NOW - 3 * H + 120)
      .map((t) => run("a", t));
    const a = computeAdherence(routines, runs, "m1", NOW, 300);
    expect(a.truncated).toBe(false);
    expect(a.rows.map((r) => r.routineId)).toEqual(["b", "a"]);
    const [beta, alpha] = a.rows;
    expect(beta).toMatchObject({ expected: 4, ran: 0, missed: 4, medianLagSecs: null, lastMissedAt: NOW - H });
    expect(alpha).toMatchObject({ expected: 24, ran: 23, missed: 1, medianLagSecs: 120, lastMissedAt: NOW - 3 * H });
    expect(a).toMatchObject({ expected: 28, ran: 23, missed: 5, medianLagSecs: 120 });
  });

  it("skips fires before creation or a current snooze, and every machine when unknown", () => {
    const routines = [
      routine({ id: "new", created_at: NOW - 2 * H - 1 }),
      routine({ id: "snoozed", snoozed_until: NOW + H }),
      routine({ id: "far", machines: ["m9"] }),
    ];
    const a = computeAdherence(routines, [], undefined, NOW, 300);
    expect(a.rows.map((r) => [r.routineId, r.expected])).toEqual([
      ["far", 24],
      ["new", 2],
    ]);
  });

  it("shrinks the window to the oldest fetched run when the sample is full", () => {
    const runs = [run("a", NOW - 2 * H), run("a", NOW - H)];
    const a = computeAdherence([routine()], runs, "m1", NOW, 2);
    expect(a.truncated).toBe(true);
    expect(a.from).toBe(NOW - 2 * H);
    expect(a.rows[0]).toMatchObject({ expected: 2, missed: 0 });
  });
});

describe("ScheduleAdherence", () => {
  const renderPanel = (a: ReturnType<typeof computeAdherence>) =>
    render(
      <MemoryRouter>
        <ScheduleAdherence adherence={a} />
      </MemoryRouter>,
    );

  it("renders nothing when no fire was expected", () => {
    const { container } = renderPanel(computeAdherence([], [], "m1", NOW, 300));
    expect(container).toBeEmptyDOMElement();
  });

  it("summarizes fleet adherence and lists per-routine misses", () => {
    renderPanel(computeAdherence([routine({ created_at: NOW - 4 * H - 1 })], [run("a", NOW - 4 * H + 30)], "m1", NOW, 300));
    expect(screen.getByText("SCHEDULE ADHERENCE · LAST 24H")).toBeInTheDocument();
    expect(screen.getByText(/1\/4 fires ran — 3 missed/)).toBeInTheDocument();
    expect(screen.getAllByText("25%")).toHaveLength(2);
    expect(screen.getByRole("link", { name: "Alpha" })).toHaveAttribute("href", "/routines?history=a");
    expect(screen.queryByText(/oldest run in the fetched sample/)).not.toBeInTheDocument();
  });

  it("flags a shortened window and a clean record", () => {
    renderPanel(computeAdherence([routine()], [run("a", NOW - 2 * H), run("a", NOW - H)], "m1", NOW, 2));
    expect(screen.getByText("SCHEDULE ADHERENCE · LAST ~2H")).toBeInTheDocument();
    expect(screen.getByText(/oldest run in the fetched sample/)).toBeInTheDocument();
    expect(screen.getByText(/2\/2 fires ran — 0 missed/)).toBeInTheDocument();
  });
});

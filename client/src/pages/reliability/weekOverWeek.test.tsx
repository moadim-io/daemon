import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { computeWeekOverWeek, WEEK_SECS } from "./weekOverWeekMath";
import { WeekOverWeek } from "./WeekOverWeek";

const NOW = 10 * WEEK_SECS;
const DAY = WEEK_SECS / 7;

function run(
  routine_id: string,
  started_at: number,
  status: FleetRunSummary["status"],
  durationSecs: number | null = 100,
): FleetRunSummary {
  return {
    routine_id,
    routine_title: routine_id.toUpperCase(),
    workbench: `wb-${started_at}`,
    started_at,
    started_at_local: "",
    finished_at: durationSecs === null ? null : started_at + durationSecs,
    finished_at_local: "",
    status,
    exit_code: status === "success" ? 0 : 1,
  };
}

/** `n` runs on consecutive days going back from `endAgo` days before NOW. */
function days(id: string, endAgo: number, statuses: FleetRunSummary["status"][], secs = 100) {
  return statuses.map((s, i) => run(id, NOW - (endAgo + i) * DAY - 1, s, secs));
}

const ok = (n: number) => Array<FleetRunSummary["status"]>(n).fill("success");

describe("computeWeekOverWeek", () => {
  it("is empty with no runs", () => {
    const r = computeWeekOverWeek([], NOW, 300);
    expect(r.changes).toEqual([]);
    expect(r.fleetCurrent).toEqual({ runs: 0, failures: 0, rate: null, p50Secs: null });
    expect(r.priorPartial).toBe(false);
  });

  it("assigns verdicts and sorts worst first", () => {
    const runs = [
      // a: 100% → 50% — worse by rate
      ...days("a", 0, ["failed", "success", "failed", "success"]),
      ...days("a", 7, ok(4)),
      // b: same rate, p50 100 s → 200 s — worse by duration
      ...days("b", 0, ok(3), 200),
      ...days("b", 7, ok(3), 100),
      // c: ran last week only — quiet
      ...days("c", 7, ok(2)),
      // d: this week only — new
      ...days("d", 0, ok(1)),
      // e: 50% → 100% — better
      ...days("e", 0, ok(2)),
      ...days("e", 7, ["failed", "success"]),
      // f: p50 200 s → 100 s — better by duration
      ...days("f", 0, ok(3), 100),
      ...days("f", 7, ok(3), 200),
      // g: unchanged — steady
      ...days("g", 0, ok(3)),
      ...days("g", 7, ok(3)),
      // ignored: running, unknown, older than 14 days, in the future
      run("g", NOW - DAY, "running", null),
      run("g", NOW - DAY, "unknown", null),
      run("z", NOW - 3 * WEEK_SECS, "failed"),
      run("z", NOW + 10, "failed"),
    ];
    const r = computeWeekOverWeek(runs, NOW, 300);
    expect(r.changes.map((c) => [c.routineId, c.verdict])).toEqual([
      ["a", "worse"],
      ["b", "worse"],
      ["c", "quiet"],
      ["d", "new"],
      ["f", "better"],
      ["e", "better"],
      ["g", "steady"],
    ]);
    const a = r.changes[0];
    expect(a?.current).toEqual({ runs: 4, failures: 2, rate: 0.5, p50Secs: 100 });
    expect(a?.rateDeltaPp).toBe(-50);
    expect(r.changes[1]?.p50Ratio).toBe(2);
    expect(r.changes[2]?.current.rate).toBeNull();
    expect(r.changes[2]?.rateDeltaPp).toBeNull();
    expect(r.fleetCurrent.failures).toBe(2);
    expect(r.fleetPrior.failures).toBe(1);
  });

  it("skips p50 under the timed-run minimum or with bad timestamps", () => {
    const runs = [
      ...days("a", 0, ok(2)),
      run("a", NOW - DAY * 3, "success", -5),
      ...days("a", 7, ok(3), 0),
    ];
    const [a] = computeWeekOverWeek(runs, NOW, 300).changes;
    expect(a?.current.p50Secs).toBeNull();
    expect(a?.prior.p50Secs).toBe(0);
    expect(a?.p50Ratio).toBeNull();
    expect(a?.verdict).toBe("steady");
  });

  it("flags a truncated sample that does not reach the prior window", () => {
    const runs = days("a", 0, ok(3));
    expect(computeWeekOverWeek(runs, NOW, 3).priorPartial).toBe(true);
    expect(computeWeekOverWeek(runs, NOW, 4).priorPartial).toBe(false);
    expect(computeWeekOverWeek([...runs, run("a", NOW - 3 * WEEK_SECS, "success")], NOW, 4).priorPartial).toBe(false);
  });
});

function renderPanel(runs: FleetRunSummary[], limit = 300) {
  return render(
    <MemoryRouter>
      <WeekOverWeek report={computeWeekOverWeek(runs, NOW, limit)} />
    </MemoryRouter>,
  );
}

describe("WeekOverWeek", () => {
  it("renders nothing with no runs in either window", () => {
    const { container } = renderPanel([]);
    expect(container.firstChild).toBeNull();
  });

  it("opens and lists deltas when something got worse", () => {
    const { container } = renderPanel([
      ...days("a", 0, ["failed", "success", "failed", "success"], 200),
      ...days("a", 7, ok(4), 100),
    ]);
    expect(container.querySelector("details")?.open).toBe(true);
    expect(screen.getByText("WORSE")).toBeInTheDocument();
    expect(screen.getByText(/fleet 100% → 50% \(-50pp\) · 1 worse or quiet/)).toBeInTheDocument();
    expect(screen.getByText("4 → 4 · 2 failed")).toBeInTheDocument();
    expect(screen.getByText(/\(\+100%\)/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "A" })).toHaveAttribute("href", "/routines?history=a");
  });

  it("stays collapsed when steady, shows no-change deltas and the partial note", () => {
    const { container } = renderPanel([...days("g", 0, ok(3)), ...days("g", 7, ok(3))], 6);
    expect(container.querySelector("details")?.open).toBe(false);
    expect(screen.getByText("STEADY")).toBeInTheDocument();
    expect(screen.getByText(/\(±0pp\) · 0 worse or quiet · prior week partial/)).toBeInTheDocument();
    expect(screen.getByText(/\(±0%\)/)).toBeInTheDocument();
    expect(screen.getByText(/prior week is incomplete/)).toBeInTheDocument();
  });

  it("shows dashes for a quiet routine and a positive delta for a better one", () => {
    renderPanel([...days("c", 7, ok(1)), ...days("e", 0, ok(2)), ...days("e", 7, ["failed", "success"])]);
    expect(screen.getByText("QUIET")).toBeInTheDocument();
    expect(screen.getByText(/50% → 100% \(\+50pp\)/)).toBeInTheDocument();
    expect(screen.getAllByText("— → —").length).toBeGreaterThan(0);
  });
});

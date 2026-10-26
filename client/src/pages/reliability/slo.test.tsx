import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import {
  computeSlo,
  DEFAULT_SLO_TARGET,
  loadSloTarget,
  saveSloTarget,
  SHORT_WINDOW_SECS,
  SLO_WINDOW_SECS,
  targetLabel,
} from "./sloMath";
import { SloBudget } from "./SloBudget";

const NOW = 10 * SLO_WINDOW_SECS;
const HOUR = 3600;

function run(routine_id: string, agoSecs: number, status: FleetRunSummary["status"]): FleetRunSummary {
  const started_at = NOW - agoSecs;
  return {
    routine_id,
    routine_title: routine_id.toUpperCase(),
    workbench: `wb-${routine_id}-${started_at}`,
    started_at,
    started_at_local: "",
    finished_at: started_at + 60,
    finished_at_local: "",
    status,
    exit_code: status === "success" ? 0 : 1,
  };
}

/** `statuses` on consecutive hours, starting `fromAgoHours` hours before NOW. */
function hourly(id: string, fromAgoHours: number, statuses: FleetRunSummary["status"][]) {
  return statuses.map((s, i) => run(id, (fromAgoHours + i) * HOUR, s));
}

const ok = (n: number) => Array<FleetRunSummary["status"]>(n).fill("success");

describe("computeSlo", () => {
  it("is empty with no runs", () => {
    expect(computeSlo([], NOW, 0.95, 300)).toEqual({ budgets: [], exhaustedCount: 0, partial: false });
  });

  it("derives budgets and sorts worst status first", () => {
    const runs = [
      // ok: 20 runs, 0 failures, all old (no 24h burn)
      ...hourly("ok", 48, ok(20)),
      // risk: 100 runs, 4 of 5 allowed failures spent, all outside 24h
      ...hourly("risk", 48, [...ok(96), "failed", "failed", "failed", "failed"]),
      // fast: 40 old successes, then 1 fail of 4 in the last 24h
      ...hourly("fast", 48, ok(40)),
      ...hourly("fast", 1, ["failed", ...ok(3)]),
      // gone: 2 of 10 failed → overspent
      ...hourly("gone", 30, ["failed", "failed", ...ok(8)]),
      // ignored: running, outside window, and in the future
      run("ok", 1, "running"),
      run("ok", SLO_WINDOW_SECS + 1, "failed"),
      run("ok", -10, "failed"),
    ];
    const r = computeSlo(runs, NOW, 0.95, 300);
    expect(r.budgets.map((b) => [b.routineId, b.status])).toEqual([
      ["gone", "exhausted"],
      ["fast", "fast-burn"],
      ["risk", "at-risk"],
      ["ok", "ok"],
    ]);
    expect(r.exhaustedCount).toBe(1);
    const [gone, fast, risk, okB] = r.budgets;
    expect(gone?.allowed).toBeCloseTo(0.5);
    expect(gone?.budgetLeft).toBeCloseTo(-3);
    expect(gone?.burn7d).toBeCloseTo(4);
    expect(fast?.burn24h).toBeCloseTo(5);
    expect(risk?.budgetLeft).toBeCloseTo(0.2);
    expect(risk?.status).toBe("at-risk");
    expect(okB).toMatchObject({ runs: 20, failures: 0, budgetLeft: 1, burn7d: 0, burn24h: null });
  });

  it("does not call a clean routine exhausted and breaks ties by title", () => {
    const runs = [...hourly("b", 2, ok(2)), ...hourly("a", 2, ok(2))];
    const r = computeSlo(runs, NOW, 0.999, 300);
    expect(r.budgets.map((b) => [b.routineId, b.status, b.burn24h])).toEqual([
      ["a", "ok", 0],
      ["b", "ok", 0],
    ]);
  });

  it("flags a capped sample that does not reach the window start", () => {
    const runs = hourly("a", 1, ok(3));
    expect(computeSlo(runs, NOW, 0.95, 3).partial).toBe(true);
    expect(computeSlo(runs, NOW, 0.95, 4).partial).toBe(false);
    expect(computeSlo([run("a", SLO_WINDOW_SECS + SHORT_WINDOW_SECS, "success"), ...runs], NOW, 0.95, 4).partial).toBe(
      false,
    );
  });
});

describe("SLO target storage", () => {
  afterEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("round-trips a known target and defaults otherwise", () => {
    expect(loadSloTarget()).toBe(DEFAULT_SLO_TARGET);
    saveSloTarget(0.99);
    expect(loadSloTarget()).toBe(0.99);
    localStorage.setItem("moadim.client.reliability.slo", "0.5");
    expect(loadSloTarget()).toBe(DEFAULT_SLO_TARGET);
  });

  it("survives unavailable storage", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(() => saveSloTarget(0.9)).not.toThrow();
    expect(loadSloTarget()).toBe(DEFAULT_SLO_TARGET);
  });

  it("labels targets", () => {
    expect(targetLabel(0.95)).toBe("95%");
    expect(targetLabel(0.999)).toBe("99.9%");
  });
});

describe("SloBudget", () => {
  const renderPanel = (runs: FleetRunSummary[], fetchLimit = 300, onTargetChange = vi.fn()) =>
    render(
      <MemoryRouter>
        <SloBudget report={computeSlo(runs, NOW, 0.95, fetchLimit)} target={0.95} onTargetChange={onTargetChange} />
      </MemoryRouter>,
    );

  it("renders nothing without finished runs", () => {
    const { container } = renderPanel([]);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows budgets, burn rates and changes the target", () => {
    const onTargetChange = vi.fn();
    renderPanel([...hourly("gone", 2, ["failed", "failed", ...ok(8)]), ...hourly("ok", 48, ok(20))], 22, onTargetChange);
    expect(screen.getByText(/SLO 95% · 7d · 1 exhausted · 1 need attention · window partial/)).toBeInTheDocument();
    expect(screen.getByText("EXHAUSTED")).toBeInTheDocument();
    expect(screen.getAllByText("OK")).toHaveLength(2);
    expect(screen.getByText("2 / 0.5 of 10 runs")).toBeInTheDocument();
    expect(screen.getByText("0 / 1.0 of 20 runs")).toBeInTheDocument();
    expect(screen.getByLabelText("GONE budget left")).toHaveAttribute("value", "0");
    expect(screen.getByText("0%")).toBeInTheDocument();
    expect(screen.queryByText(/-\d+%/)).not.toBeInTheDocument();
    expect(screen.getAllByText("4.0×")).toHaveLength(2);
    expect(screen.getAllByText("—")).toHaveLength(1);
    expect(screen.getByText(/window is incomplete/)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(/SLO TARGET/), { target: { value: "0.99" } });
    expect(onTargetChange).toHaveBeenCalledWith(0.99);
  });

  it("formats large allowances without decimals and stays closed when all ok", () => {
    const { container } = renderPanel(ok(200).map((s, i) => run("big", (i + 1) * 1800, s)));
    expect(screen.getByText("0 / 10 of 200 runs")).toBeInTheDocument();
    expect(container.querySelector("details")).not.toHaveAttribute("open");
  });
});

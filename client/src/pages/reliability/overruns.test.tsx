import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { computeReliability } from "./reliabilityStats";
import { computeOverruns } from "./overrunMath";
import { LongRunning } from "./LongRunning";

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
    exit_code: status === "running" ? null : status === "success" ? 0 : 1,
  };
}

const NOW = 100_000;

function report(runs: FleetRunSummary[]) {
  return computeOverruns(runs, computeReliability(runs), NOW);
}

const history = (id: string, secs: number) => [1, 2, 3].map((i) => run(id, i * 1_000, "success", secs));

describe("computeOverruns", () => {
  it("is empty with no running runs", () => {
    expect(report(history("a", 100))).toEqual({ overruns: [], runningCount: 0, noBaselineCount: 0 });
  });

  it("flags runs past 1.5× p95, worst ratio first", () => {
    const r = report([
      run("a", NOW - 1_000, "running", null), // 10× a's 100 s p95
      run("b", NOW - 1_000, "running", null), // 2× b's 500 s p95
      run("c", NOW - 120, "running", null), // 1.2× c's 100 s p95 — within tolerance
      ...history("a", 100),
      ...history("b", 500),
      ...history("c", 100),
    ]);
    expect(r.runningCount).toBe(3);
    expect(r.noBaselineCount).toBe(0);
    expect(r.overruns.map((o) => [o.routineId, o.ratio])).toEqual([
      ["a", 10],
      ["b", 2],
    ]);
    expect(r.overruns[0]).toMatchObject({ elapsedSecs: 1_000, p95Secs: 100, workbench: `wb-${NOW - 1_000}` });
  });

  it("needs a baseline of finished runs", () => {
    const r = report([run("a", NOW - 5_000, "running", null), run("a", 1_000, "success", 10), run("z", 1, "running", null)]);
    expect(r).toEqual({ overruns: [], runningCount: 2, noBaselineCount: 2 });
  });

  it("ignores young runs even against an instant baseline", () => {
    const r = report([run("a", NOW - 30, "running", null), run("b", NOW - 90, "running", null), ...history("a", 0), ...history("b", 0)]);
    expect(r.overruns.map((o) => [o.routineId, o.ratio, o.p95Secs])).toEqual([["b", 90, 0]]);
  });
});

describe("LongRunning", () => {
  const view = (runs: FleetRunSummary[]) =>
    render(
      <MemoryRouter>
        <LongRunning report={report(runs)} />
      </MemoryRouter>,
    );

  it("renders nothing without running runs", () => {
    const { container } = view(history("a", 100));
    expect(container).toBeEmptyDOMElement();
  });

  it("lists overruns with a link to the live run", () => {
    view([run("a", NOW - 1_000, "running", null), run("b", NOW - 200, "running", null), run("z", 1, "running", null), ...history("a", 100), ...history("b", 100)]);
    expect(screen.getByText("LONG-RUNNING NOW")).toBeInTheDocument();
    expect(screen.getByText("2 of 3 running over baseline · 1 without baseline")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "A" })).toHaveAttribute("href", `/runs/a/wb-${NOW - 1_000}`);
    expect(screen.getByText("10.0×")).toHaveClass("failed");
    expect(screen.getByText("2.0×")).toHaveClass("running");
  });

  it("shows a collapsed summary when nothing overruns", () => {
    view([run("a", NOW - 10, "running", null), ...history("a", 100)]);
    expect(screen.getByText("0 of 1 running over baseline")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});

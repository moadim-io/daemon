import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { RunsGantt } from "./RunsGantt";
import { buildGantt } from "./ganttMath";

function run(o: Partial<FleetRunSummary>): FleetRunSummary {
  return {
    routine_id: "r1",
    routine_title: "Nightly Audit",
    workbench: "wb1",
    started_at: 100,
    started_at_local: "",
    finished_at: 200,
    finished_at_local: "",
    status: "success",
    exit_code: 0,
    ...o,
  };
}

const RUNS = [
  run({}),
  run({ workbench: "wb2", routine_id: "r2", routine_title: "Digest", started_at: 150, finished_at: null, status: "running" }),
];

function renderGantt(runs: FleetRunSummary[], cap: number | undefined) {
  return render(
    <MemoryRouter>
      <RunsGantt gantt={buildGantt(runs, 1000)} cap={cap} />
    </MemoryRouter>,
  );
}

describe("RunsGantt", () => {
  it("renders nothing without lanes", () => {
    const { container } = renderGantt([], 2);
    expect(container).toBeEmptyDOMElement();
  });

  it("draws one lane per routine with bars linking to the run", () => {
    renderGantt(RUNS, 4);
    expect(screen.getByText("Digest")).toBeInTheDocument();
    const bar = screen.getByRole("link", { name: /Nightly Audit: started .*, success/ });
    expect(bar).toHaveAttribute("href", "/runs/r1/wb1");
    expect(screen.getByRole("link", { name: /Digest: .*running, running/ })).toHaveClass("runs-gantt-bar", "running");
    expect(screen.getByText(/peak concurrency 2 \/ cap 4/)).toBeInTheDocument();
    expect(document.querySelector(".runs-gantt-cap")).toBeNull();
  });

  it("warns and draws the cap line when peak reaches the cap", () => {
    renderGantt(RUNS, 2);
    expect(screen.getByText(/runs may have queued/)).toHaveClass("c-amber");
    expect(document.querySelector(".runs-gantt-cap")).not.toBeNull();
  });

  it("labels an unbounded cap and a loading cap", () => {
    const { unmount } = renderGantt(RUNS, 0);
    expect(screen.getByText(/peak concurrency 2 \(no cap\)/)).toBeInTheDocument();
    unmount();
    renderGantt(RUNS, undefined);
    expect(screen.getByText("peak concurrency 2")).toBeInTheDocument();
  });
});

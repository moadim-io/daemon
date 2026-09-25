import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CapacityForecast } from "./CapacityForecast";
import { buildForecast } from "./forecastMath";
import { routine, run } from "./forecastMath.test";

const NOW = Math.floor(new Date(2026, 5, 21, 12, 0, 0).getTime() / 1000);
const ROUTINES = [
  routine({ schedule: "0 13 * * *" }),
  routine({ id: "b", title: "Beta", schedule: "30 13 * * *" }),
];
const RUNS = [
  run({ finished_at: 7_200 }),
  run({ routine_id: "b", started_at: NOW - 60, finished_at: null, status: "running" }),
];

function renderForecast(cap: number | undefined) {
  return render(<CapacityForecast forecast={buildForecast(ROUTINES, RUNS, NOW, cap ?? 0)} cap={cap} />);
}

describe("CapacityForecast", () => {
  it("renders nothing when nothing is projected", () => {
    const { container } = render(<CapacityForecast forecast={buildForecast([], [], NOW, 1)} cap={1} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("draws lanes with median/default estimates and lists over-cap windows", () => {
    renderForecast(1);
    expect(screen.getByText(/projected peak 2 \/ cap 1 — 1 over-cap window$/)).toHaveClass("c-amber");
    expect(screen.getByRole("img", { name: /Alpha: fires .*, ~2h 0m median/ })).toHaveClass("runs-gantt-bar");
    expect(screen.getByRole("img", { name: /Beta: running now, ~10m \(default, no history\)/ })).toHaveClass("running");
    expect(screen.getByRole("img", { name: /Beta: fires .*default/ })).toHaveClass("est");
    expect(screen.getByRole("list", { name: "Over-cap windows" })).toHaveTextContent("13:30–13:40 · 2 projected Alpha, Beta");
    expect(document.querySelector(".runs-gantt-cap")).not.toBeNull();
    expect(document.querySelector(".runs-gantt-level.over")).not.toBeNull();
  });

  it("labels an unbounded and a loading cap without warnings", () => {
    const { unmount } = renderForecast(0);
    expect(screen.getByText(/projected peak 2 \(no cap\)$/)).not.toHaveClass("c-amber");
    expect(screen.queryByRole("list")).toBeNull();
    unmount();
    renderForecast(undefined);
    expect(screen.getByText(/^projected peak 2$/)).toBeInTheDocument();
    expect(document.querySelector(".runs-gantt-cap")).toBeNull();
  });

  it("pluralizes multiple over-cap windows", () => {
    const routines = [...ROUTINES, routine({ id: "c", title: "Gamma", schedule: "30 13,20 * * *" })];
    const runs = [run({ finished_at: 7_200 }), run({ routine_id: "b", finished_at: 36_000 })];
    render(<CapacityForecast forecast={buildForecast(routines, runs, NOW, 1)} cap={1} />);
    expect(screen.getByText(/over-cap windows$/)).toBeInTheDocument();
  });
});

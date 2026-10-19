import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { computeIncidents } from "./incidentMath";
import { FailureIncidents } from "./FailureIncidents";

function run(
  routine_id: string,
  started_at: number,
  status: FleetRunSummary["status"],
  finished_at: number | null = started_at + 60,
): FleetRunSummary {
  return {
    routine_id,
    routine_title: routine_id.toUpperCase(),
    workbench: `wb-${started_at}`,
    started_at,
    started_at_local: "",
    finished_at,
    finished_at_local: "",
    status,
    exit_code: status === "success" ? 0 : 1,
  };
}

const NOW = 100_000;

describe("computeIncidents", () => {
  it("is empty without failures", () => {
    expect(computeIncidents([run("a", 10, "success")], NOW)).toEqual({
      incidents: [],
      openCount: 0,
      mttrSecs: null,
      longestSecs: null,
    });
  });

  it("groups consecutive failures, resolves on the next success, and ignores running runs", () => {
    // Newest-first input, as the API returns it.
    const r = computeIncidents(
      [
        run("a", 5_000, "running", null),
        run("a", 4_000, "success"),
        run("a", 3_000, "failed"),
        run("a", 2_000, "failed"),
        run("a", 1_000, "success"),
      ],
      NOW,
    );
    expect(r.incidents).toEqual([
      {
        routineId: "a",
        routineTitle: "A",
        openedAt: 2_000,
        resolvedAt: 4_060,
        durationSecs: 2_060,
        failedRuns: 2,
        firstWorkbench: "wb-2000",
        truncated: false,
      },
    ]);
    expect(r.openCount).toBe(0);
    expect(r.mttrSecs).toBe(2_060);
    expect(r.longestSecs).toBe(2_060);
  });

  it("marks ongoing and sample-truncated incidents and orders ongoing first", () => {
    const r = computeIncidents(
      [
        run("b", 9_000, "failed"),
        run("a", 3_000, "success", null),
        run("a", 1_000, "failed"),
        run("a", 500, "success"),
        run("a", 100, "failed"),
      ],
      NOW,
    );
    expect(r.incidents.map((i) => [i.routineId, i.openedAt, i.resolvedAt, i.durationSecs, i.truncated])).toEqual([
      ["b", 9_000, null, NOW - 9_000, true],
      ["a", 1_000, 3_000, 2_000, false],
      ["a", 100, 560, 460, true],
    ]);
    expect(r.openCount).toBe(1);
    expect(r.mttrSecs).toBe(1_230);
    expect(r.longestSecs).toBe(2_000);
  });
});

describe("FailureIncidents", () => {
  const renderPanel = (runs: FleetRunSummary[]) =>
    render(
      <MemoryRouter>
        <FailureIncidents report={computeIncidents(runs, NOW)} />
      </MemoryRouter>,
    );

  it("renders nothing without incidents", () => {
    const { container } = renderPanel([run("a", 10, "success")]);
    expect(container).toBeEmptyDOMElement();
  });

  it("lists ongoing and resolved incidents with the MTTR summary", () => {
    renderPanel([run("b", 9_000, "failed"), run("a", 2_000, "success"), run("a", 1_000, "failed"), run("a", 1, "success")]);
    expect(screen.getByText("1 open")).toHaveClass("c-red");
    expect(screen.getByText(/2 total · MTTR 17m · longest 17m/)).toBeInTheDocument();
    expect(screen.getByText("ONGOING")).toBeInTheDocument();
    expect(screen.getByText("RESOLVED", { selector: "span" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "A" })).toHaveAttribute("href", "/routines?history=a");
    const links = screen.getAllByTitle("Open the first failed run");
    expect(links[0]).toHaveAttribute("href", "/runs/b/wb-9000");
    expect(links[0]).toHaveTextContent(/^≥ /);
    expect(links[1]).toHaveAttribute("href", "/runs/a/wb-1000");
    expect(links[1]).not.toHaveTextContent("≥");
  });

  it("shows a dash for MTTR when nothing has recovered yet", () => {
    renderPanel([run("a", 1_000, "failed")]);
    expect(screen.getByText(/MTTR — · longest —/)).toBeInTheDocument();
  });

  it("shows no open incidents without the alert color", () => {
    renderPanel([run("a", 2_000, "success"), run("a", 1_000, "failed")]);
    expect(screen.getByText("0 open")).not.toHaveClass("c-red");
  });
});

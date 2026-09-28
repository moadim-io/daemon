import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { computeFailureCauses, exitCodeMeaning } from "./exitCodeMath";
import { FailureCauses } from "./FailureCauses";

function run(
  routine_id: string,
  started_at: number,
  status: FleetRunSummary["status"],
  exit_code: number | null,
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
    exit_code,
  };
}

describe("exitCodeMeaning", () => {
  it("names known codes, signals, and unknowns", () => {
    expect(exitCodeMeaning(null)).toBe("no exit code recorded");
    expect(exitCodeMeaning(137)).toBe("killed (SIGKILL / OOM)");
    expect(exitCodeMeaning(129)).toBe("signal 1");
    expect(exitCodeMeaning(3)).toBe("routine-specific");
    expect(exitCodeMeaning(200)).toBe("routine-specific");
  });
});

describe("computeFailureCauses", () => {
  it("is empty without failures", () => {
    expect(computeFailureCauses([run("a", 1, "success", 0)])).toEqual({ causes: [], totalFailed: 0 });
  });

  it("groups by code, most frequent first, ties newest first, tracks routines and last run", () => {
    const r = computeFailureCauses([
      run("a", 100, "failed", 137),
      run("b", 300, "failed", 137),
      run("a", 200, "failed", 137),
      run("c", 500, "failed", 1),
      run("c", 50, "failed", null, null),
      run("a", 900, "success", 0),
    ]);
    expect(r.totalFailed).toBe(5);
    expect(r.causes.map((c) => c.code)).toEqual([137, 1, null]);
    expect(r.causes[0]).toEqual({
      code: 137,
      count: 3,
      routines: [
        { id: "a", title: "A", count: 2 },
        { id: "b", title: "B", count: 1 },
      ],
      last: { routineId: "b", workbench: "wb-300", at: 360 },
    });
    expect(r.causes[2]?.last.at).toBe(50);
  });
});

describe("FailureCauses", () => {
  it("renders nothing without failures", () => {
    const { container } = render(<FailureCauses report={{ causes: [], totalFailed: 0 }} />);
    expect(container.innerHTML).toBe("");
  });

  it("renders rows with meanings, shares, routine overflow and links", () => {
    const report = computeFailureCauses([
      run("a", 1, "failed", 137),
      run("a", 2, "failed", 137),
      run("b", 3, "failed", 137),
      run("c", 4, "failed", 137),
      run("d", 5, "failed", 137),
      run("e", 6, "failed", null),
    ]);
    render(
      <MemoryRouter>
        <FailureCauses report={report} />
      </MemoryRouter>,
    );
    expect(screen.getByText("FAILURE CAUSES")).toBeInTheDocument();
    expect(screen.getByText(/6 failed runs · 2 distinct exit codes/)).toBeInTheDocument();
    expect(screen.getByText("killed (SIGKILL / OOM)")).toBeInTheDocument();
    expect(screen.getByText("5 (83%)")).toBeInTheDocument();
    expect(screen.getByText("?")).toBeInTheDocument();
    expect(screen.getByText("×2")).toBeInTheDocument();
    expect(screen.getByText("+1 more")).toBeInTheDocument();
    expect(screen.getByText("A").closest("a")).toHaveAttribute("href", "/routines?history=a");
    const last = screen.getAllByTitle("Open the newest failed run with this exit code");
    expect(last[0]).toHaveAttribute("href", "/runs/d/wb-5");
  });

  it("uses singular wording for one code", () => {
    render(
      <MemoryRouter>
        <FailureCauses report={computeFailureCauses([run("a", 1, "failed", 1)])} />
      </MemoryRouter>,
    );
    expect(screen.getByText(/1 distinct exit code$/)).toBeInTheDocument();
  });
});

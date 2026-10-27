import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { computeDigest, loadOrSeedLastSeen, saveLastSeen } from "./visitDigest";
import { SinceLastVisit } from "./SinceLastVisit";

function run(routine_id: string, finished: number | null, status: FleetRunSummary["status"]): FleetRunSummary {
  return {
    routine_id,
    routine_title: routine_id.toUpperCase(),
    workbench: `wb-${routine_id}-${finished ?? "live"}`,
    started_at: (finished ?? 10_000) - 10,
    started_at_local: "",
    finished_at: finished,
    finished_at_local: "",
    status,
    exit_code: null,
  };
}

const SINCE = 1_000;

describe("computeDigest", () => {
  it("counts only runs finished after the baseline", () => {
    const runs = [run("a", 2_000, "success"), run("b", null, "running"), run("a", 500, "failed")];
    expect(computeDigest(runs, SINCE, 300)).toEqual({ failed: 0, succeeded: 1, routines: [], truncated: false });
  });

  it("groups failures per routine, still-failing first, and marks recoveries", () => {
    const runs = [
      run("ok", 3_000, "success"),
      run("bad", 2_500, "failed"),
      run("ok", 2_000, "failed"),
      run("bad", 1_500, "failed"),
      run("ok", 1_200, "failed"),
    ];
    const d = computeDigest(runs, SINCE, 300);
    expect(d.failed).toBe(4);
    expect(d.succeeded).toBe(1);
    expect(d.routines.map((r) => [r.routineId, r.failures, r.recovered, r.latestFailure.finished_at])).toEqual([
      ["bad", 2, false, 2_500],
      ["ok", 2, true, 2_000],
    ]);
  });

  it("orders same-state routines by newest failure", () => {
    const d = computeDigest([run("x", 2_000, "failed"), run("y", 3_000, "failed")], SINCE, 300);
    expect(d.routines.map((r) => r.routineId)).toEqual(["y", "x"]);
  });

  it("flags truncation when the capped fetch never reaches the baseline", () => {
    expect(computeDigest([run("a", 2_000, "success")], SINCE, 1).truncated).toBe(true);
    expect(computeDigest([run("a", 900, "success")], SINCE, 1).truncated).toBe(false);
    expect(computeDigest([], SINCE, 0).truncated).toBe(false);
  });
});

describe("last-seen storage", () => {
  afterEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("seeds a first visit with now, then reads the stored value", () => {
    expect(loadOrSeedLastSeen(123)).toBe(123);
    expect(loadOrSeedLastSeen(999)).toBe(123);
    saveLastSeen(456);
    expect(loadOrSeedLastSeen(999)).toBe(456);
  });

  it("falls back to now when storage throws", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(loadOrSeedLastSeen(77)).toBe(77);
    expect(() => saveLastSeen(1)).not.toThrow();
  });
});

describe("SinceLastVisit", () => {
  const renderDigest = (runs: FleetRunSummary[], onMarkSeen = () => {}) =>
    render(
      <MemoryRouter>
        <SinceLastVisit digest={computeDigest(runs, SINCE, 1)} sinceSecs={SINCE} onMarkSeen={onMarkSeen} />
      </MemoryRouter>,
    );

  it("says nothing new and hides the acknowledge button when empty", () => {
    renderDigest([run("a", 500, "success")]);
    expect(screen.getByText("No runs finished since your last visit.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Mark as seen" })).toBeNull();
  });

  it("lists failing and recovered routines with run links and acknowledges", () => {
    const onMarkSeen = vi.fn();
    renderDigest([run("ok", 3_000, "success"), run("bad", 2_500, "failed"), run("ok", 2_000, "failed")], onMarkSeen);
    expect(screen.getByText("2 failed")).toBeTruthy();
    expect(screen.getByText(/older runs not loaded/)).toBeTruthy();
    expect(screen.getByText("FAILING")).toBeTruthy();
    expect(screen.getByText("RECOVERED")).toBeTruthy();
    expect(screen.getByRole("link", { name: "BAD" }).getAttribute("href")).toBe("/routines?history=bad");
    const runLinks = screen.getAllByRole("link").filter((l) => l.getAttribute("href")?.startsWith("/runs/"));
    expect(runLinks.map((l) => l.getAttribute("href"))).toEqual(["/runs/bad/wb-bad-2500", "/runs/ok/wb-ok-2000"]);
    fireEvent.click(screen.getByRole("button", { name: "Mark as seen" }));
    expect(onMarkSeen).toHaveBeenCalledOnce();
  });

  it("shows counts without a table when only successes finished", () => {
    renderDigest([run("a", 2_000, "success"), run("b", 900, "success")]);
    expect(screen.getByText("0 failed")).toBeTruthy();
    expect(screen.queryByRole("table")).toBeNull();
  });
});

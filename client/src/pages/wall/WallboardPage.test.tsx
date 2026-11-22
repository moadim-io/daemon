import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { WallboardPage } from "./WallboardPage";
import { HISTORY_LEN, buildTiles, tileCounts } from "./wallboardMath";

const NOW = new Date("2026-11-06T12:00:00Z");
const NOW_S = Math.floor(NOW.getTime() / 1000);

function routine(id: string, extra: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id,
    title: `T-${id}`,
    schedule: "0 * * * *",
    enabled: true,
    machines: ["m1"],
    agent_registered: true,
    flag_count: 0,
    ...extra,
  } as RoutineResponse;
}

function run(routine_id: string, started_at: number, status: FleetRunSummary["status"], exit_code?: number): FleetRunSummary {
  return {
    routine_id,
    routine_title: `T-${routine_id}`,
    workbench: `wb-${routine_id}-${started_at}`,
    started_at,
    started_at_local: "",
    status,
    exit_code,
  };
}

describe("buildTiles", () => {
  it("derives each state and sorts worst-first", () => {
    const routines = [
      routine("ok"),
      routine("fail"),
      routine("failnocode"),
      routine("run"),
      routine("idle"),
      routine("snz", { skip_runs: 2 }),
      routine("flags", { flag_count: 1 }),
      routine("off", { enabled: false }),
      routine("auto", { enabled: false, auto_disabled_reason: "3 failures" }),
    ];
    const runs = [
      run("ok", NOW_S - 100, "success"),
      run("ok", NOW_S - 200, "failed"),
      run("fail", NOW_S - 50, "failed", 2),
      run("failnocode", NOW_S - 50, "failed"),
      run("run", NOW_S - 10, "running"),
      run("run", NOW_S - 500, "failed"),
      run("off", NOW_S - 10, "failed"),
    ];
    const tiles = buildTiles(routines, runs, NOW);
    expect(tiles.map((t) => [t.id, t.state])).toEqual([
      ["fail", "failing"],
      ["failnocode", "failing"],
      ["auto", "attention"],
      ["flags", "attention"],
      ["run", "running"],
      ["ok", "ok"],
      ["idle", "idle"],
      ["snz", "snoozed"],
    ]);
    const byId = Object.fromEntries(tiles.map((t) => [t.id, t]));
    expect(byId.fail?.reason).toBe("last run failed · exit 2");
    expect(byId.failnocode?.reason).toBe("last run failed");
    expect(byId.flags?.reason).toBe("open flags");
    expect(byId.ok?.history).toEqual(["failed", "success"]);
    expect(byId.ok?.latest?.started_at).toBe(NOW_S - 100);
    expect(byId.ok?.nextAt).toEqual(new Date("2026-11-06T13:00:00Z"));
    expect(byId.snz?.nextAt).toBeUndefined();
    expect(byId.auto?.nextAt).toBeUndefined();
    expect(byId.idle?.lastStartedAt).toBeUndefined();
  });

  it("caps history and counts every state", () => {
    const runs = Array.from({ length: 15 }, (_, i) => run("a", NOW_S - i, "success"));
    const tiles = buildTiles([routine("a")], runs, NOW);
    expect(tiles[0]?.history).toHaveLength(HISTORY_LEN);
    expect(tileCounts(tiles)).toEqual({ failing: 0, attention: 0, running: 0, ok: 1, idle: 0, snoozed: 0 });
  });
});

function renderPage(seed: (qc: QueryClient) => void) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  seed(qc);
  render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={["/wall"]}>
        <Routes>
          <Route path="/wall" element={<WallboardPage />} />
          <Route path="/" element={<p>home page</p>} />
          <Route path="/runs/:id/:wb" element={<p>run page</p>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("WallboardPage", () => {
  beforeEach(() => localStorage.clear());

  it("renders tiles, counts and links a tile to its latest run", () => {
    renderPage((qc) => {
      qc.setQueryData(["routines", {}], [routine("a"), routine("b")]);
      qc.setQueryData(["routines", "runs", 300], [run("a", NOW_S, "failed", 1)]);
    });
    expect(screen.getByText("T-a")).toBeTruthy();
    expect(screen.getByText("last run failed · exit 1")).toBeTruthy();
    expect(screen.getByText("no runs yet")).toBeTruthy();
    expect(screen.getByText(/refreshes every 15s/)).toBeTruthy();
    fireEvent.click(screen.getByText("T-a"));
    expect(screen.getByText("run page")).toBeTruthy();
  });

  it("shows the empty state, requests fullscreen, and exits on Escape", () => {
    const requestFullscreen = vi.fn(() => Promise.resolve());
    Object.defineProperty(document.documentElement, "requestFullscreen", { value: requestFullscreen, configurable: true });
    renderPage((qc) => {
      qc.setQueryData(["routines", {}], [routine("off", { enabled: false })]);
      qc.setQueryData(["routines", "runs", 300], []);
    });
    expect(screen.getByText("No enabled routines to show.")).toBeTruthy();
    fireEvent.click(screen.getByText("FULLSCREEN"));
    expect(requestFullscreen).toHaveBeenCalled();
    fireEvent.keyDown(window, { key: "x" });
    expect(screen.queryByText("home page")).toBeNull();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByText("home page")).toBeTruthy();
  });

  it("shows loading, then the refresh error banner", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new Error("offline"))));
    renderPage(() => undefined);
    expect(screen.getByText("Loading…")).toBeTruthy();
    expect(await screen.findByText(/Data refresh failed/)).toBeTruthy();
    vi.unstubAllGlobals();
  });
});

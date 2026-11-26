import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { UsagePage } from "./UsagePage";

const TODAY = new Date(2026, 10, 25).getTime() / 1000;
const SIXTY_DAYS_AGO = new Date(2026, 8, 26).getTime() / 1000;

function run(routineId: string, startedAt: number, secs: number, status: FleetRunSummary["status"] = "success") {
  return {
    routine_id: routineId,
    routine_title: routineId,
    workbench: `wb-${routineId}-${startedAt}`,
    started_at: startedAt,
    started_at_local: "",
    status,
    exit_code: null,
    finished_at: startedAt + secs,
    finished_at_local: null,
  } as FleetRunSummary;
}

const routines = [
  { id: "alpha", title: "Alpha", agent: "claude" },
  { id: "beta", title: "Beta", agent: "codex" },
] as RoutineResponse[];

function renderPage(opts: { routines?: RoutineResponse[]; runs?: FleetRunSummary[]; url?: string } = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (opts.routines !== undefined) queryClient.setQueryData(["routines", {}], opts.routines);
  if (opts.runs !== undefined) queryClient.setQueryData(["routines", "runs", 2000], opts.runs);
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[opts.url ?? "/usage"]}>
        <UsagePage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

// The stat tile comes before the table header of the same name.
const statVal = (label: string) => screen.getAllByText(label)[0]!.nextElementSibling;
const firstCol = () => [...document.querySelectorAll("tbody tr > td:first-child > a")];

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(2026, 10, 25, 12));
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("UsagePage", () => {
  it("shows a spinner before the queries have loaded", () => {
    renderPage();
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows an error state when a query fails", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("boom", { status: 500 }));
    renderPage();
    expect(await screen.findByText("FAILED TO LOAD")).toBeInTheDocument();
  });

  it("shows the empty state with no runs", () => {
    renderPage({ routines, runs: [] });
    expect(screen.getByText("NO AGENT TIME YET")).toBeInTheDocument();
    expect(screen.getByText("no run started in the last 60 days")).toBeInTheDocument();
  });

  it("renders KPIs, one bar per day, and the top consumers", () => {
    renderPage({
      routines,
      runs: [run("alpha", TODAY + 100, 3600), run("beta", TODAY + 200, 1800, "failed"), run("alpha", TODAY - 40 * 86_400, 3600)],
    });
    expect(screen.getByRole("heading", { name: "Usage" })).toBeInTheDocument();
    expect(statVal("AGENT TIME")).toHaveTextContent("1h 30m");
    expect(statVal("VS PREVIOUS 30D")).toHaveTextContent("+50%");
    expect(statVal("VS PREVIOUS 30D")).toHaveClass("c-red");
    expect(statVal("RUNS")).toHaveTextContent("2");
    expect(screen.getAllByRole("listitem")).toHaveLength(30);
    expect(firstCol().map((a) => a.textContent)).toEqual(["Alpha", "Beta"]);
    expect(firstCol()[0]).toHaveAttribute("href", "/routines/alpha");
    expect(screen.getByText("67%")).toBeInTheDocument();
    expect(screen.getByText("new")).toBeInTheDocument();
  });

  it("groups by agent and switches the range via the URL", () => {
    renderPage({ routines, runs: [run("alpha", TODAY + 100, 600), run("beta", TODAY + 200, 60)] });
    fireEvent.click(screen.getByRole("button", { name: "BY AGENT" }));
    expect(screen.getByRole("columnheader", { name: "AGENT" })).toBeInTheDocument();
    expect(firstCol()[0]).toHaveAttribute("href", "/routines?agent=claude");
    fireEvent.click(screen.getByRole("button", { name: "7D" }));
    expect(screen.getByRole("button", { name: "7D" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getAllByRole("listitem")).toHaveLength(7);
    fireEvent.click(screen.getByRole("button", { name: "BY ROUTINE" }));
    fireEvent.click(screen.getByRole("button", { name: "30D" }));
    expect(screen.getByRole("columnheader", { name: "ROUTINE" })).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")).toHaveLength(30);
  });

  it("shows previous-period-only routines with zero current time", () => {
    renderPage({ routines, runs: [run("alpha", TODAY - 40 * 86_400, 600)] });
    expect(screen.getByText("—")).toBeInTheDocument();
    expect(screen.getAllByText("-100%")).toHaveLength(2);
  });

  it("warns when the capped sample does not reach the previous period", () => {
    const many = Array.from({ length: 2000 }, (_, i) => run("alpha", TODAY + i, 1));
    renderPage({ routines, runs: [...many.slice(1), run("alpha", SIXTY_DAYS_AGO + 86_400 * 31, 1)] });
    expect(screen.getByText(/Only the newest 2000 runs are loaded/)).toBeInTheDocument();
  });
});

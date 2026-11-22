import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { UptimePage } from "./UptimePage";

const TODAY = new Date(2026, 10, 16).getTime() / 1000;
const YESTERDAY = new Date(2026, 10, 15).getTime() / 1000;
const TOMORROW = new Date(2026, 10, 17).getTime() / 1000;

function routine(id: string): RoutineResponse {
  return { id, title: id } as RoutineResponse;
}

function run(routineId: string, startedAt: number, status: FleetRunSummary["status"] = "success"): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: routineId,
    workbench: `wb-${startedAt}`,
    started_at: startedAt,
    started_at_local: "",
    status,
    exit_code: null,
    finished_at: null,
    finished_at_local: null,
  };
}

function renderPage(opts: { routines?: RoutineResponse[]; runs?: FleetRunSummary[]; url?: string } = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (opts.routines !== undefined) queryClient.setQueryData(["routines", {}], opts.routines);
  if (opts.runs !== undefined) queryClient.setQueryData(["routines", "runs", 2000], opts.runs);
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[opts.url ?? "/uptime"]}>
        <UptimePage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const fleet = {
  routines: [routine("alpha"), routine("beta"), routine("idle"), routine("live")],
  runs: [
    run("alpha", TODAY + 100, "failed"),
    run("alpha", YESTERDAY + 100),
    run("beta", TODAY + 200),
    run("beta", TODAY + 300, "running"),
    run("live", TODAY + 400, "running"),
  ],
};

const strips = () => [...document.querySelectorAll(".uptime-strip")];
const rowNames = () =>
  [...document.querySelectorAll("tbody tr:not(.uptime-fleet) > td:first-child > a")].map((a) => a.textContent);

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(2026, 10, 16, 12));
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("UptimePage", () => {
  it("shows a spinner before the queries have loaded", () => {
    renderPage();
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows an error state when a query fails", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("boom", { status: 500 }));
    renderPage();
    expect(await screen.findByText("FAILED TO LOAD")).toBeInTheDocument();
  });

  it("shows the empty state with no routines", () => {
    renderPage({ routines: [], runs: [] });
    expect(screen.getByText("NO ROUTINES YET")).toBeInTheDocument();
  });

  it("renders KPIs, a fleet row, and one day bar per day per routine", () => {
    renderPage(fleet);

    expect(screen.getByRole("heading", { name: "Uptime" })).toBeInTheDocument();
    expect(screen.getByText("FLEET UPTIME").nextElementSibling).toHaveTextContent("66.6%");
    expect(screen.getByText("DAYS WITH FAILURES").nextElementSibling).toHaveTextContent("1 / 30");
    expect(screen.getByText("ROUTINES AT 100%").nextElementSibling).toHaveTextContent("1");
    expect(screen.getByText("FINISHED RUNS").nextElementSibling).toHaveTextContent("3");

    expect(screen.getByText("All routines")).toBeInTheDocument();
    expect(rowNames()).toEqual(["alpha", "beta", "idle", "live"]);
    expect(screen.getByRole("link", { name: "alpha" })).toHaveAttribute("href", "/routines/alpha");
    expect(screen.getByText("50.0%")).toBeInTheDocument();
    expect(strips()).toHaveLength(5);
    for (const strip of strips()) expect(strip.children).toHaveLength(30);
    expect(screen.queryByText(/may be incomplete/)).toBeNull();
  });

  it("links each day bar to that day's runs, scoped to the routine", () => {
    renderPage(fleet);

    expect(screen.getByRole("link", { name: /: 0 succeeded, 1 failed$/ })).toHaveAttribute(
      "href",
      `/runs?q=alpha&from=${TODAY}&to=${TOMORROW}`,
    );
    expect(screen.getByRole("link", { name: /: 1 succeeded, 1 failed, 1\+ running$/ })).toHaveAttribute(
      "href",
      `/runs?from=${TODAY}&to=${TOMORROW}`,
    );
    const live = screen.getByRole("link", { name: /no finished runs \(running now\)$/ });
    expect(live).toHaveClass("live");
    expect(screen.getAllByRole("img", { name: /: no finished runs$/ }).length).toBeGreaterThanOrEqual(30);
  });

  it("switches the range and keeps it in the URL", () => {
    renderPage(fleet);
    fireEvent.click(screen.getByRole("button", { name: "7D" }));
    expect(screen.getByRole("button", { name: "7D" })).toHaveAttribute("aria-pressed", "true");
    expect(strips()[0]!.children).toHaveLength(7);
    expect(screen.getByText("LAST 7 DAYS")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "30D" }));
    expect(strips()[0]!.children).toHaveLength(30);
  });

  it("restores the range from ?days=", () => {
    renderPage({ ...fleet, url: "/uptime?days=90" });
    expect(strips()[0]!.children).toHaveLength(90);
  });

  it("narrows rows with the search box and shows a no-match state", () => {
    renderPage(fleet);
    const search = screen.getByRole("searchbox", { name: "Search routines" });

    fireEvent.change(search, { target: { value: "BET" } });
    expect(rowNames()).toEqual(["beta"]);

    fireEvent.change(search, { target: { value: "nothing-matches" } });
    expect(screen.getByText("NO MATCHING ROUTINES")).toBeInTheDocument();
  });

  it("warns when the capped run sample may not cover the whole range", () => {
    renderPage({
      routines: [routine("alpha")],
      runs: Array.from({ length: 2000 }, (_, i) => run("alpha", TODAY + i)),
    });
    expect(screen.getByText(/newest 2000 runs are loaded/)).toBeInTheDocument();
  });
});

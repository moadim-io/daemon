import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary } from "../../api/hooks";
import { RunsPage } from "./RunsPage";

function run(overrides: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    routine_id: "r1",
    routine_title: "Nightly Audit",
    workbench: "nightly-audit-1000",
    started_at: Math.floor(Date.now() / 1000) - 10,
    started_at_local: "",
    finished_at: Math.floor(Date.now() / 1000),
    finished_at_local: "",
    status: "success",
    exit_code: 0,
    ...overrides,
  };
}

function renderPage(runs?: FleetRunSummary[]) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (runs !== undefined) queryClient.setQueryData(["routines", "runs", 100], runs);
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return queryClient;
}

describe("RunsPage", () => {
  it("shows a spinner before the query has loaded", () => {
    renderPage();
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows an empty state with no runs at all", () => {
    renderPage([]);
    expect(screen.getByText("NO RUNS")).toBeInTheDocument();
    expect(screen.getByText("no runs yet")).toBeInTheDocument();
  });

  it("lists fetched runs with routine link and status", () => {
    renderPage([
      run({ workbench: "wb1", routine_title: "Nightly Audit", status: "success" }),
      run({ workbench: "wb2", routine_title: "Weekly Digest", status: "failed", exit_code: 1 }),
    ]);

    expect(screen.getByRole("heading", { name: "Runs" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Nightly Audit" })).toHaveAttribute(
      "href",
      "/routines?history=r1",
    );
    expect(screen.getByText("Weekly Digest")).toBeInTheDocument();
    expect(screen.getByText("Showing 2 of 2 fetched")).toBeInTheDocument();
  });

  it("filters the list by status", () => {
    renderPage([
      run({ workbench: "wb1", routine_title: "Nightly Audit", status: "success" }),
      run({ workbench: "wb2", routine_title: "Weekly Digest", status: "failed" }),
    ]);

    fireEvent.change(screen.getByLabelText("Status filter"), { target: { value: "failed" } });

    expect(screen.getByText("Showing 1 of 2 fetched")).toBeInTheDocument();
    expect(screen.queryByText("Nightly Audit")).not.toBeInTheDocument();
    expect(screen.getByText("Weekly Digest")).toBeInTheDocument();
  });

  it("filters the list by routine title search", () => {
    renderPage([
      run({ workbench: "wb1", routine_title: "Nightly Audit" }),
      run({ workbench: "wb2", routine_title: "Weekly Digest" }),
    ]);

    fireEvent.change(screen.getByLabelText("Search runs"), { target: { value: "weekly" } });

    expect(screen.getByText("Showing 1 of 2 fetched")).toBeInTheDocument();
    expect(screen.getByText("Weekly Digest")).toBeInTheDocument();
  });

  it("shows a no-matches empty state distinct from no-runs-yet", () => {
    renderPage([run({ routine_title: "Nightly Audit" })]);
    fireEvent.change(screen.getByLabelText("Search runs"), { target: { value: "nonexistent" } });
    expect(screen.getByText("NO RUNS")).toBeInTheDocument();
    expect(screen.getByText("no runs match the current filters")).toBeInTheDocument();
  });

  it("offers a load-more action once the fetch limit is hit", () => {
    const runs = Array.from({ length: 100 }, (_, i) => run({ workbench: `wb${i}`, started_at: 1_000 + i }));
    renderPage(runs);
    expect(screen.getByRole("button", { name: "Load more" })).toBeInTheDocument();
  });

  it("does not offer load-more under the fetch limit", () => {
    renderPage([run()]);
    expect(screen.queryByRole("button", { name: "Load more" })).not.toBeInTheDocument();
  });
});

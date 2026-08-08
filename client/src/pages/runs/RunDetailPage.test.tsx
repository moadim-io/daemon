import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { Routine, RunSummary } from "../../api/hooks";
import { RunDetailPage } from "./RunDetailPage";

function run(overrides: Partial<RunSummary> = {}): RunSummary {
  return {
    workbench: "nightly-audit-1000",
    started_at: 1_000,
    started_at_local: "",
    finished_at: 1_060,
    finished_at_local: "",
    status: "success",
    exit_code: 0,
    retention_expires_at: 90_000,
    ...overrides,
  };
}

function routine(overrides: Partial<Routine> = {}): Routine {
  return {
    id: "r1",
    schedule: "0 3 * * *",
    title: "Nightly Audit",
    agent: "claude",
    enabled: true,
    source: "cli",
    created_at: 0,
    updated_at: 0,
    ...overrides,
  };
}

function renderPage(opts: { routineId?: string; workbench?: string; runs?: RunSummary[]; routineData?: Routine } = {}) {
  const { routineId = "r1", workbench = "nightly-audit-1000", runs, routineData } = opts;
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (runs !== undefined) queryClient.setQueryData(["routines", routineId, "runs"], runs);
  if (routineData !== undefined) queryClient.setQueryData(["routines", routineId], routineData);
  queryClient.setQueryData(["routines", routineId, "runs", workbench, "log"], "hello from the log\n");

  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[`/runs/${routineId}/${workbench}`]}>
        <Routes>
          <Route path="/runs/:routineId/:workbench" element={<RunDetailPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return queryClient;
}

describe("RunDetailPage", () => {
  it("shows a spinner before the runs query has loaded", () => {
    renderPage();
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows a not-found state when the workbench isn't in the routine's run list", () => {
    renderPage({ runs: [run({ workbench: "some-other-run" })] });
    expect(screen.getByText("RUN NOT FOUND")).toBeInTheDocument();
  });

  it("resolves the routine title standalone (deep link) via useRoutine", () => {
    renderPage({ runs: [run()], routineData: routine({ title: "Nightly Audit" }) });
    expect(screen.getByRole("link", { name: "← Nightly Audit" })).toHaveAttribute(
      "href",
      "/routines?history=r1",
    );
  });

  it("falls back to the routine id as a heading while the routine hasn't resolved", () => {
    renderPage({ runs: [run()] });
    expect(screen.getByRole("link", { name: "← r1" })).toBeInTheDocument();
  });

  it("renders run metadata and the log", () => {
    renderPage({ runs: [run({ status: "failed", exit_code: 42 })] });
    expect(screen.getByText("FAILED")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText("hello from the log")).toBeInTheDocument();
  });

  it("disables older/newer navigation at the ends of the run list, and enables between them", () => {
    const runs = [run({ workbench: "wb-newest", started_at: 3_000 }), run({ workbench: "wb-mid", started_at: 2_000 }), run({ workbench: "wb-oldest", started_at: 1_000 })];
    renderPage({ workbench: "wb-mid", runs });

    expect(screen.getByRole("button", { name: "← Older run" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Newer run →" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "← Older run" }));
    expect(screen.getByText("RUN / wb-oldest")).toBeInTheDocument();
  });

  it("disables the older-run button on the oldest run", () => {
    const runs = [run({ workbench: "wb-newest", started_at: 2_000 }), run({ workbench: "wb-oldest", started_at: 1_000 })];
    renderPage({ workbench: "wb-oldest", runs });
    expect(screen.getByRole("button", { name: "← Older run" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Newer run →" })).toBeEnabled();
  });
});

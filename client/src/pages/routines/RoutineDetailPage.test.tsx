import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RoutineResponse, RunSummary } from "../../api/hooks";
import { ToastProvider } from "../../shell/toasts";
import { ToastStack } from "../../shell/ToastStack";
import { RoutineDetailPage } from "./RoutineDetailPage";

const triggerMutate = vi.fn();
vi.mock("../../api/hooks", async (importActual) => ({
  ...(await importActual<typeof import("../../api/hooks")>()),
  useTriggerRoutine: () => ({ mutate: triggerMutate, isPending: false }),
}));

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "r1",
    schedule: "0 3 * * *",
    title: "Nightly Audit",
    agent: "claude",
    enabled: true,
    source: "cli",
    created_at: 0,
    updated_at: 0,
    agent_registered: true,
    agent_command_available: true,
    agent_setup_available: true,
    file_path: "/x/routine.toml",
    slug: "nightly-audit",
    rel_path: "ops/nightly-audit",
    flag_count: 0,
    is_running: false,
    env_keys: [],
    ...overrides,
  } as RoutineResponse;
}

function run(i: number, overrides: Partial<RunSummary> = {}): RunSummary {
  const started = 1_000_000 - i * 1_000;
  return {
    workbench: `wb-${i}`,
    started_at: started,
    started_at_local: "",
    finished_at: started + 60,
    finished_at_local: "",
    status: "success",
    exit_code: 0,
    ...overrides,
  };
}

function renderPage(opts: { routineData?: RoutineResponse; runs?: RunSummary[] } = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  if (opts.routineData !== undefined) queryClient.setQueryData(["routines", "r1"], opts.routineData);
  if (opts.runs !== undefined) queryClient.setQueryData(["routines", "r1", "runs"], opts.runs);
  render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <MemoryRouter initialEntries={["/routines/r1"]}>
          <Routes>
            <Route path="/routines/:id" element={<RoutineDetailPage />} />
            <Route path="/runs/:routineId/:workbench" element={<div>RUN PAGE</div>} />
          </Routes>
        </MemoryRouter>
        <ToastStack />
      </ToastProvider>
    </QueryClientProvider>,
  );
  return queryClient;
}

afterEach(() => {
  triggerMutate.mockReset();
  localStorage.clear();
});

describe("RoutineDetailPage", () => {
  it("shows a spinner while the routine loads", () => {
    renderPage();
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows a not-found state when the routine fetch fails", async () => {
    renderPage();
    expect(await screen.findByText("ROUTINE NOT FOUND")).toBeInTheDocument();
  });

  it("renders KPIs, configuration, upcoming fires and the capped recent-runs table", () => {
    // Newest six runs take 600s, older six 60s: the newer half is far slower, so P95 flags it.
    const runs = Array.from({ length: 12 }, (_, i) => run(i, i < 6 ? { finished_at: 1_000_000 - i * 1_000 + 600 } : {}));
    renderPage({
      routineData: routine({
        goal: "Audit the fleet\nevery night",
        model: "opus",
        machines: ["mac-mini"],
        repositories: [{ repository: "owner/repo" }] as RoutineResponse["repositories"],
        tags: ["ops"],
        timezone: "UTC",
        ttl_secs: 86_400,
        max_runtime_secs: 600,
        env_keys: ["TOKEN"],
        schedule_descriptions: ["At 03:00 AM"],
        flag_count: 2,
        is_running: true,
      }),
      runs,
    });

    expect(screen.getByText("ROUTINE / Nightly Audit")).toBeInTheDocument();
    expect(screen.getByText(/Audit the fleet/)).toBeInTheDocument();
    expect(screen.getByText("RUNNING")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "▶ RUN NOW" })).toBeDisabled();
    expect(screen.getByText("▲ slower")).toBeInTheDocument();
    expect(screen.getByText("claude · opus")).toBeInTheDocument();
    expect(screen.getByText("owner/repo")).toBeInTheDocument();
    expect(screen.getByText("TOKEN")).toBeInTheDocument();
    expect(screen.getByText("ops/nightly-audit")).toBeInTheDocument();
    expect(screen.getByText("At 03:00 AM")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "2" })).toHaveAttribute("href", "/flags");
    expect(screen.getByRole("link", { name: "FULL HISTORY" })).toHaveAttribute("href", "/routines?history=r1");
    expect(within(screen.getByRole("region", { name: "Upcoming fires" })).getAllByRole("listitem")).toHaveLength(5);
    // Header row + RECENT_COUNT (10) body rows, even though 12 runs exist.
    expect(screen.getAllByRole("row")).toHaveLength(11);
  });

  it("opens the run permalink when a duration bar is clicked", async () => {
    renderPage({ routineData: routine(), runs: [run(0)] });
    const chart = screen.getByRole("list", { name: "Run duration chart" });
    fireEvent.click(within(chart).getAllByRole("listitem")[0]!);
    expect(await screen.findByText("RUN PAGE")).toBeInTheDocument();
  });

  it("explains a paused routine has no upcoming fires and no runs", () => {
    renderPage({ routineData: routine({ enabled: false }), runs: [] });
    expect(screen.getByText("not scheduled while paused or snoozed")).toBeInTheDocument();
    expect(screen.getByText("NO RUNS YET")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "▶ RUN NOW" })).toBeDisabled();
  });

  it("says when an enabled schedule has no upcoming fire", () => {
    renderPage({ routineData: routine({ schedule: "@reboot" }), runs: [] });
    expect(screen.getByText("no upcoming fires")).toBeInTheDocument();
  });

  it("toggles the pin", () => {
    renderPage({ routineData: routine(), runs: [] });
    fireEvent.click(screen.getByRole("button", { name: "☆ PIN" }));
    expect(screen.getByRole("button", { name: "★ PINNED" })).toHaveAttribute("aria-pressed", "true");
  });

  it("triggers the routine and toasts on success and on failure", () => {
    renderPage({ routineData: routine(), runs: [] });
    fireEvent.click(screen.getByRole("button", { name: "▶ RUN NOW" }));
    expect(triggerMutate).toHaveBeenCalledWith("r1", expect.anything());
    const callbacks = triggerMutate.mock.calls[0]![1] as { onSuccess: () => void; onError: (e: Error) => void };
    act(() => callbacks.onSuccess());
    expect(screen.getByText('Triggered "Nightly Audit"')).toBeInTheDocument();
    act(() => callbacks.onError(new Error("boom")));
    expect(screen.getByText("Trigger failed: boom")).toBeInTheDocument();
  });

  it("refetches the routine and its runs on refresh", () => {
    const queryClient = renderPage({ routineData: routine(), runs: [] });
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(queryClient.isFetching()).toBe(2);
  });

  it("shows the runs error while keeping the routine", async () => {
    renderPage({ routineData: routine() });
    expect(await screen.findByText(/^Error:/)).toBeInTheDocument();
    expect(screen.getByText("ROUTINE / Nightly Audit")).toBeInTheDocument();
  });
});

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { MachinesPage } from "./MachinesPage";

function routine(id: string, overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id,
    title: id,
    agent: "claude",
    model: null,
    schedule: "* * * * *",
    prompt: "",
    repositories: [],
    machines: ["box-1"],
    enabled: true,
    source: "",
    created_at: 0,
    updated_at: 0,
    last_manual_trigger_at: null,
    last_scheduled_trigger_at: null,
    snoozed_until: null,
    skip_runs: null,
    power_saving: false,
    ttl_secs: null,
    tags: [],
    agent_registered: true,
    agent_command_available: true,
    agent_setup_available: true,
    is_running: false,
    file_path: "",
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

function seed(
  queryClient: QueryClient,
  opts: { machines?: string[]; current?: string; routines?: RoutineResponse[]; runs?: FleetRunSummary[] },
) {
  if (opts.machines !== undefined) queryClient.setQueryData(["machines"], opts.machines);
  if (opts.current !== undefined) queryClient.setQueryData(["machine"], { name: opts.current });
  if (opts.routines !== undefined) queryClient.setQueryData(["routines", {}], opts.routines);
  if (opts.runs !== undefined) queryClient.setQueryData(["routines", "runs", 300], opts.runs);
}

function renderPage(seedOpts: Parameters<typeof seed>[1] = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  seed(queryClient, seedOpts);
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <MachinesPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return queryClient;
}

describe("MachinesPage", () => {
  it("shows a spinner before every query has loaded", () => {
    renderPage();
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows the empty state once loaded with no known machines", () => {
    renderPage({ machines: [], current: "", routines: [], runs: [] });
    expect(screen.getByText("NO MACHINES YET")).toBeInTheDocument();
  });

  it("lists a machine with its routine, run, and health rollups", () => {
    renderPage({
      machines: ["box-1", "box-2"],
      current: "box-1",
      routines: [
        routine("r1", { machines: ["box-1"], is_running: true }),
        routine("r2", { machines: ["box-1"], enabled: false }),
        routine("r3", { machines: [] }),
      ],
      runs: [
        {
          routine_id: "r1",
          routine_title: "r1",
          workbench: "wb1",
          started_at: 100,
          started_at_local: "12:00",
          status: "success",
          exit_code: 0,
          finished_at: 110,
          finished_at_local: "12:01",
        },
      ],
    });

    expect(screen.getByRole("heading", { name: "Machines" })).toBeInTheDocument();
    expect(screen.getByText("box-1")).toBeInTheDocument();
    expect(screen.getByText("box-2")).toBeInTheDocument();
    expect(screen.getByText("(this machine)")).toBeInTheDocument();
    expect(screen.getByText("2 routines")).toBeInTheDocument();
    // one routine (r3) targets no machine at all
    expect(screen.getByText("UNASSIGNED ROUTINES").nextElementSibling).toHaveTextContent("1");
  });
});

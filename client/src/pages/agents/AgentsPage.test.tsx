import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { AgentsPage } from "./AgentsPage";

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
    folder: null,
    slug: "routine",
    rel_path: "routine",
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

function renderPage(opts: { agents?: string[]; routines?: RoutineResponse[]; runs?: FleetRunSummary[] } = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (opts.agents !== undefined) queryClient.setQueryData(["agents"], opts.agents);
  if (opts.routines !== undefined) queryClient.setQueryData(["routines", {}], opts.routines);
  if (opts.runs !== undefined) queryClient.setQueryData(["routines", "runs", 300], opts.runs);
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <AgentsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("AgentsPage", () => {
  it("shows a spinner before every query has loaded", () => {
    renderPage();
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows the empty state once loaded with no agents", () => {
    renderPage({ agents: [], routines: [], runs: [] });
    expect(screen.getByText("NO AGENTS YET")).toBeInTheDocument();
  });

  it("lists each agent with health, rollups, and a link to its filtered routines", () => {
    renderPage({
      agents: ["claude", "codex"],
      routines: [
        routine("r1", { agent: "claude", model: "opus", is_running: true }),
        routine("r2", { agent: "gone" }),
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
          finished_at: 160,
          finished_at_local: "12:01",
        },
      ],
    });

    expect(screen.getByRole("heading", { name: "Agents" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "claude" })).toHaveAttribute("href", "/routines?agent=claude");
    expect(screen.getByText("codex")).toBeInTheDocument();
    expect(screen.getByText("UNREGISTERED")).toBeInTheDocument();
    expect(screen.getByText("UNUSED")).toBeInTheDocument();
    expect(screen.getByText("opus")).toBeInTheDocument();
    expect(screen.getByText("12:00")).toBeInTheDocument();
    expect(screen.getAllByText("1 routine")).toHaveLength(2);
    expect(screen.getByText("UNHEALTHY").nextElementSibling).toHaveTextContent("1");
    expect(screen.getByText("ENABLED ROUTINES BLOCKED").nextElementSibling).toHaveTextContent("1");
  });
});

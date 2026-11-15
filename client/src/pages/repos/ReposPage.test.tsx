import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { ReposPage } from "./ReposPage";

const API = "https://github.com/acme/api.git";
const WEB = "git@github.com:acme/web.git";

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

function run(routineId: string, startedAt: number, overrides: Partial<FleetRunSummary> = {}): FleetRunSummary {
  return {
    routine_id: routineId,
    routine_title: routineId,
    workbench: `wb-${startedAt}`,
    started_at: startedAt,
    started_at_local: `t${startedAt}`,
    status: "success",
    exit_code: 0,
    finished_at: startedAt + 20,
    finished_at_local: "",
    ...overrides,
  };
}

function renderPage(opts: { routines?: RoutineResponse[]; runs?: FleetRunSummary[] } = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (opts.routines !== undefined) queryClient.setQueryData(["routines", {}], opts.routines);
  if (opts.runs !== undefined) queryClient.setQueryData(["routines", "runs", 300], opts.runs);
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <ReposPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

/** Two repositories: `acme/api` (2 routines, one failing, auto-pull off) and `acme/web` (1 routine). */
const fleet = {
  routines: [
    routine("build", {
      repositories: [{ repository: API, branch: "main", auto_pull: false }],
      is_running: true,
    }),
    routine("lint", {
      enabled: false,
      agent: "codex",
      repositories: [{ repository: API }, { repository: WEB, branch: "dev" }],
    }),
    routine("orphan"),
  ],
  runs: [run("build", 300, { status: "failed" }), run("lint", 100)],
};

const rowNames = () => [...document.querySelectorAll("tbody tr > td:first-child > a")].map((a) => a.textContent);

beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe("ReposPage", () => {
  it("shows a spinner before every query has loaded", () => {
    renderPage();
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows an error state when the routine list fails to load", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("boom", { status: 500 }));
    renderPage();
    expect(await screen.findByText("FAILED TO LOAD")).toBeInTheDocument();
  });

  it("shows the empty state when no routine declares a repository", () => {
    renderPage({ routines: [routine("orphan")], runs: [] });
    expect(screen.getByText("NO REPOSITORIES YET")).toBeInTheDocument();
    expect(screen.getByText("ROUTINES WITHOUT A REPOSITORY").nextElementSibling).toHaveTextContent("1");
  });

  it("lists each repository with its rollups, a filtered link, and an auto-pull badge", () => {
    renderPage(fleet);

    expect(screen.getByRole("heading", { name: "Repositories" })).toBeInTheDocument();
    expect(screen.getByText("REPOSITORIES").nextElementSibling).toHaveTextContent("2");
    expect(screen.getByText("WITH FAILING ROUTINES").nextElementSibling).toHaveTextContent("1");
    expect(screen.getByText("ROUTINES WITHOUT A REPOSITORY").nextElementSibling).toHaveTextContent("1");

    const api = screen.getByRole("link", { name: "acme/api" });
    expect(api).toHaveAttribute("href", `/routines?repo=${encodeURIComponent(API)}`);
    expect(api).toHaveAttribute("title", API);
    expect(screen.getByText("AUTO-PULL OFF ×1")).toBeInTheDocument();
    expect(screen.getByText("2 routines")).toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: "lint" })[0]).toHaveAttribute("href", "/routines?history=lint");
    expect(screen.getByText("default, main")).toBeInTheDocument();
    expect(screen.getByText("claude, codex")).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();
    expect(screen.getByText("t300")).toBeInTheDocument();
  });

  it("narrows the table with the search box and shows a no-match state", () => {
    renderPage(fleet);
    const search = screen.getByRole("searchbox", { name: "Search repositories" });

    fireEvent.change(search, { target: { value: "WEB" } });
    expect(rowNames()).toEqual(["acme/web"]);

    fireEvent.change(search, { target: { value: "nothing-matches" } });
    expect(screen.getByText("NO MATCHING REPOSITORIES")).toBeInTheDocument();
  });

  it("reorders rows when a sortable header is clicked", () => {
    renderPage(fleet);
    expect(rowNames()).toEqual(["acme/api", "acme/web"]);

    fireEvent.click(screen.getByRole("button", { name: "ROUTINES" }));
    expect(rowNames()).toEqual(["acme/api", "acme/web"]);
    fireEvent.click(screen.getByRole("button", { name: /ROUTINES/ }));
    expect(rowNames()).toEqual(["acme/web", "acme/api"]);

    fireEvent.click(screen.getByRole("button", { name: /ROUTINES/ }));
    expect(rowNames()).toEqual(["acme/api", "acme/web"]);
    expect(localStorage.getItem("moadim.client.repos.sort")).toBeNull();
  });

  it("sorts by the failing and success-rate columns", () => {
    renderPage(fleet);
    fireEvent.click(screen.getByRole("button", { name: "FAILING" }));
    expect(rowNames()).toEqual(["acme/api", "acme/web"]);
    fireEvent.click(screen.getByRole("button", { name: /FAILING/ }));
    expect(rowNames()).toEqual(["acme/web", "acme/api"]);

    fireEvent.click(screen.getByRole("button", { name: "SUCCESS RATE" }));
    expect(rowNames()).toEqual(["acme/api", "acme/web"]);
    expect(localStorage.getItem("moadim.client.repos.sort")).toBe(JSON.stringify({ key: "rate", dir: "asc" }));
  });

  it("sorts by the repository, enabled, running, and p50 columns without losing a row", () => {
    renderPage(fleet);
    for (const label of ["REPOSITORY", "ENABLED", "RUNNING NOW", "P50"]) {
      fireEvent.click(screen.getByRole("button", { name: label }));
      expect(rowNames()).toHaveLength(2);
      fireEvent.click(screen.getByRole("button", { name: new RegExp(label) }));
      expect(rowNames()).toHaveLength(2);
    }
  });
});

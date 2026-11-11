import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { loadPins, savePins } from "../../lib/pins";
import { PinnedPanel } from "./PinnedPanel";

function routine(id: string, title: string, overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id,
    title,
    agent: "claude",
    model: null,
    schedule: "0 * * * *",
    prompt: "",
    repositories: [],
    machines: ["m1"],
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
    slug: id,
    rel_path: id,
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

const run: FleetRunSummary = {
  routine_id: "a",
  routine_title: "Alpha",
  workbench: "wb-a",
  started_at: 1,
  started_at_local: "",
  finished_at: 2,
  finished_at_local: "",
  status: "success",
  exit_code: 0,
};

function renderPanel(routines: RoutineResponse[], onTrigger = vi.fn()) {
  render(
    <MemoryRouter>
      <PinnedPanel routines={routines} runHistory={new Map([["a", [run]]])} now={new Date()} onTrigger={onTrigger} />
    </MemoryRouter>,
  );
  return onTrigger;
}

beforeEach(() => {
  localStorage.clear();
});

describe("PinnedPanel", () => {
  it("shows an empty state that points to the Routines page", () => {
    renderPanel([routine("a", "Alpha")]);
    expect(screen.getByText("NO PINNED ROUTINES")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Routines" }).getAttribute("href")).toBe("/routines");
  });

  it("lists pinned routines in pin order and skips deleted ones", () => {
    savePins(["b", "gone", "a"]);
    renderPanel([routine("a", "Alpha"), routine("b", "Beta"), routine("c", "Gamma")]);
    const links = screen.getAllByRole("link").map((l) => l.textContent);
    expect(links).toEqual(["Beta", "Alpha"]);
    expect(screen.getByRole("link", { name: "Alpha" }).getAttribute("href")).toBe("/routines?history=a");
    expect(screen.getByRole("img", { name: "Last 1 runs" })).toBeTruthy();
    expect(screen.queryByText("Gamma")).toBeNull();
  });

  it("triggers and unpins from the row", () => {
    savePins(["a", "b"]);
    const onTrigger = renderPanel([routine("a", "Alpha"), routine("b", "Beta", { enabled: false })]);
    fireEvent.click(screen.getByRole("button", { name: "Run Alpha now" }));
    expect(onTrigger).toHaveBeenCalledWith("a");
    expect((screen.getByRole("button", { name: "Run Beta now" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Unpin Alpha" }));
    expect(loadPins()).toEqual(["b"]);
    expect(screen.queryByText("Alpha")).toBeNull();
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import type { RoutineStatusFacet } from "./filter";
import { StatsBar } from "./StatsBar";

const NOW = new Date("2026-07-17T12:00:00Z");

function routine(id: string, enabled: boolean, overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id,
    title: id,
    agent: "claude",
    model: null,
    // Fires every minute, so an enabled routine using this schedule is always "due soon".
    schedule: "* * * * *",
    prompt: "",
    repositories: [],
    machines: ["box-1"],
    enabled,
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

function renderBar(routines: RoutineResponse[], active: RoutineStatusFacet = "all") {
  const onStatus = vi.fn();
  render(<StatsBar routines={routines} now={NOW} active={active} onStatus={onStatus} />);
  return onStatus;
}

function tileValue(label: string): string {
  return screen.getByText(label).nextElementSibling?.textContent ?? "";
}

describe("StatsBar", () => {
  it("counts total/enabled/disabled", () => {
    renderBar([routine("a", true), routine("b", true), routine("c", false)]);
    expect(tileValue("TOTAL")).toBe("3");
    expect(tileValue("ENABLED")).toBe("2");
    expect(tileValue("DISABLED")).toBe("1");
  });

  it("counts due-soon, snoozed, dormant, flagged, and unregistered-agent routines", () => {
    const futureSnooze = Math.floor(NOW.getTime() / 1000) + 3600;
    renderBar([
      // Every-minute schedule, so every enabled+unsnoozed routine below also counts as "due soon".
      routine("due", true),
      routine("snoozed", true, { snoozed_until: futureSnooze }),
      routine("dormant", true, { machines: [] }),
      routine("flagged", true, { flag_count: 2 }),
      routine("unreg", true, { agent_registered: false }),
      // Disabled routines never count toward the enabled-only tiles above.
      routine("disabled-dormant", false, { machines: [] }),
    ]);
    expect(tileValue("DUE SOON")).toBe("4");
    expect(tileValue("SNOOZED")).toBe("1");
    expect(tileValue("DORMANT")).toBe("1");
    expect(tileValue("FLAGS")).toBe("2");
    expect(tileValue("UNREGISTERED AGENT")).toBe("1");
  });

  it("applies a has-* class only when its tile's count is non-zero", () => {
    renderBar([routine("a", true, { machines: [] })]);
    expect(screen.getByText("DORMANT").closest("button")).toHaveClass("has-dormant");
    expect(screen.getByText("FLAGS").closest("button")).not.toHaveClass("has-flags");
  });

  it("clicking an inactive tile selects its facet", () => {
    const onStatus = renderBar([routine("a", true)]);
    fireEvent.click(screen.getByText("ENABLED").closest("button")!);
    expect(onStatus).toHaveBeenCalledWith("enabled");
  });

  it("clicking the active tile again clears back to all", () => {
    const onStatus = renderBar([routine("a", true)], "enabled");
    fireEvent.click(screen.getByText("ENABLED").closest("button")!);
    expect(onStatus).toHaveBeenCalledWith("all");
  });

  it("marks the active tile with aria-pressed", () => {
    renderBar([routine("a", true)], "enabled");
    expect(screen.getByText("ENABLED").closest("button")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("TOTAL").closest("button")).toHaveAttribute("aria-pressed", "false");
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import { ToastProvider } from "../../shell/toasts";
import { RoutineCalendar } from "./RoutineCalendar";

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "daily-digest",
    title: "Daily digest",
    agent: "hermes",
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
    slug: "daily-digest",
    rel_path: "daily-digest",
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

describe("RoutineCalendar", () => {
  it("renders scheduled routine chips as edit buttons", () => {
    const onEdit = vi.fn();

    render(
      <ToastProvider>
        <RoutineCalendar routines={[routine()]} loading={false} onEdit={onEdit} />
      </ToastProvider>,
    );

    const editButtons = screen.getAllByRole("button", { name: "Edit Daily digest" });
    expect(editButtons.length).toBeGreaterThan(0);

    fireEvent.click(editButtons[0] as HTMLElement);
    expect(onEdit).toHaveBeenCalledWith("daily-digest");
  });
});

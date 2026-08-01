import { fireEvent, render, screen, within } from "@testing-library/react";
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

function renderCalendar(overrides: Partial<RoutineResponse> = {}) {
  const onEdit = vi.fn();
  const onTrigger = vi.fn();
  render(
    <ToastProvider>
      <RoutineCalendar routines={[routine(overrides)]} loading={false} onEdit={onEdit} onTrigger={onTrigger} />
    </ToastProvider>,
  );
  return { onEdit, onTrigger };
}

describe("RoutineCalendar", () => {
  it("renders scheduled routine chips as edit buttons", () => {
    const { onEdit } = renderCalendar();

    const editButtons = screen.getAllByRole("button", { name: "Edit Daily digest" });
    expect(editButtons.length).toBeGreaterThan(0);

    fireEvent.click(editButtons[0] as HTMLElement);
    expect(onEdit).toHaveBeenCalledWith("daily-digest");
  });

  it("opens a day-detail dialog with exact fire times and Run now actions", () => {
    const { onTrigger } = renderCalendar({ schedule: "0 9,17 * * *" });

    fireEvent.click(screen.getAllByRole("button", { name: /Open schedule details for/ })[0] as HTMLElement);

    const dialog = screen.getByRole("dialog", { name: "Calendar day details" });
    expect(within(dialog).getByText("Daily digest")).toBeInTheDocument();
    expect(within(dialog).getByText("09:00")).toBeInTheDocument();
    expect(within(dialog).getByText("17:00")).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: "Run Daily digest now" }));
    expect(onTrigger).toHaveBeenCalledWith("daily-digest");
  });

  it("opens an accessible empty day-detail dialog for days with no fires", () => {
    renderCalendar({ schedule: "0 9 1 1 *" });

    fireEvent.click(screen.getAllByRole("button", { name: /Open schedule details for/ })[0] as HTMLElement);

    expect(screen.getByRole("dialog", { name: "Calendar day details" })).toBeInTheDocument();
    expect(screen.getByText("No routine fires on this day.")).toBeInTheDocument();
  });
});

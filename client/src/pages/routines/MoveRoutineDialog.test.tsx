import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import { MoveRoutineDialog } from "./MoveRoutineDialog";

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "r1",
    title: "Daily digest",
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
    folder: "maintenance",
    slug: "daily-digest",
    rel_path: "maintenance/daily-digest",
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

describe("MoveRoutineDialog", () => {
  it("submits a filesystem folder and slug without changing the title", () => {
    const onConfirm = vi.fn();
    render(<MoveRoutineDialog routine={routine()} saving={false} onCancel={vi.fn()} onConfirm={onConfirm} />);

    fireEvent.change(screen.getByLabelText("FOLDER"), { target: { value: "hermes/learning" } });
    fireEvent.change(screen.getByLabelText("SLUG*"), { target: { value: "review-nudge" } });
    expect(screen.getByText("routines/hermes/learning/review-nudge/routine.toml")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Move"));

    expect(onConfirm).toHaveBeenCalledWith("hermes/learning", "review-nudge");
  });

  it("uses undefined folder for root moves and disables invalid slugs", () => {
    const onConfirm = vi.fn();
    render(<MoveRoutineDialog routine={routine()} saving={false} onCancel={vi.fn()} onConfirm={onConfirm} />);

    fireEvent.change(screen.getByLabelText("FOLDER"), { target: { value: "" } });
    fireEvent.change(screen.getByLabelText("SLUG*"), { target: { value: "bad/slug" } });
    expect(screen.getByText("Move")).toBeDisabled();

    fireEvent.change(screen.getByLabelText("SLUG*"), { target: { value: "root-slug" } });
    fireEvent.click(screen.getByText("Move"));
    expect(onConfirm).toHaveBeenCalledWith(undefined, "root-slug");
  });
});

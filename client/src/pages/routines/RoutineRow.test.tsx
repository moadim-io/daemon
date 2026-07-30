import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import { RoutineRow } from "./RoutineRow";

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "r1",
    title: "Nightly backup",
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
    slug: "routine",
    rel_path: "routine",
    schedule_description: null,
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

function renderRow(r: RoutineResponse) {
  render(
    <table>
      <tbody>
        <RoutineRow
          routine={r}
          now={new Date(2026, 0, 1, 12, 0, 0)}
          runs={[]}
          selected={false}
          onSelect={vi.fn()}
          onEdit={vi.fn()}
          onClone={vi.fn()}
          onDelete={vi.fn()}
          onToggle={vi.fn()}
          onTrigger={vi.fn()}
          onLogs={vi.fn()}
          onHistory={vi.fn()}
          onFlags={vi.fn()}
        />
      </tbody>
    </table>,
  );
}

describe("RoutineRow — failure circuit-breaker (issue #521)", () => {
  it("shows an AUTO-DISABLED badge with the daemon's reason as the tooltip", () => {
    renderRow(routine({ enabled: false, auto_disabled_reason: "5 consecutive failures" }));
    const badge = screen.getByText("AUTO-DISABLED");
    expect(badge).toHaveAttribute("title", "5 consecutive failures");
  });

  it("a manually-disabled routine still shows the plain DISABLED badge", () => {
    renderRow(routine({ enabled: false, auto_disabled_reason: null }));
    expect(screen.getByText("DISABLED")).toBeInTheDocument();
    expect(screen.queryByText("AUTO-DISABLED")).not.toBeInTheDocument();
  });

  it("shows no failure chip when there are no consecutive failures", () => {
    renderRow(routine({ consecutive_failures: 0, failure_threshold: 5 }));
    expect(screen.queryByText("0/5")).not.toBeInTheDocument();
  });

  it("shows a warning failure chip while short of the threshold", () => {
    renderRow(routine({ consecutive_failures: 2, failure_threshold: 5 }));
    const chip = screen.getByText("2/5");
    expect(chip).toHaveClass("failure-chip", "warning");
  });

  it("shows a critical failure chip when the next failure trips the breaker", () => {
    renderRow(routine({ consecutive_failures: 4, failure_threshold: 5 }));
    const chip = screen.getByText("4/5");
    expect(chip).toHaveClass("failure-chip", "critical");
  });
});

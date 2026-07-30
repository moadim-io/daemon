import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import { RoutineTable, type RoutineTableProps } from "./RoutineTable";

function routine(id: string, title: string, agent: string, overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id,
    title,
    agent,
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
    agent_command_available: false,
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

function baseProps(overrides: Partial<RoutineTableProps> = {}): RoutineTableProps {
  return {
    routines: [],
    loading: false,
    filterActive: false,
    now: new Date(2026, 0, 1, 12, 0, 0),
    selected: new Set(),
    onSelect: vi.fn(),
    onSelectAll: vi.fn(),
    sortCol: undefined,
    sortDir: "asc",
    groupBy: "none",
    runHistory: new Map(),
    onSort: vi.fn(),
    onEdit: vi.fn(),
    onClone: vi.fn(),
    onDelete: vi.fn(),
    onToggle: vi.fn(),
    onTrigger: vi.fn(),
    onLogs: vi.fn(),
    onHistory: vi.fn(),
    onFlags: vi.fn(),
    onClearFilters: vi.fn(),
    ...overrides,
  };
}

describe("RoutineTable — grouping", () => {
  beforeEach(() => localStorage.clear());

  const routines = [
    routine("a", "Nightly backup", "claude", { agent_registered: true }),
    routine("b", "backend/db/vacuum", "codex", { enabled: false }),
    routine("c", "backend/db/reindex", "claude", { agent_registered: false }),
  ];

  it("groupBy none renders no group headers", () => {
    render(<RoutineTable {...baseProps({ routines, groupBy: "none" })} />);
    expect(screen.queryByText(/backend\/db/)).not.toBeInTheDocument();
  });

  it("groupBy folder renders a header per folder with a rollup count", () => {
    render(<RoutineTable {...baseProps({ routines, groupBy: "folder" })} />);
    expect(screen.getByText("(root)")).toBeInTheDocument();
    expect(screen.getByText("backend/db")).toBeInTheDocument();
    expect(screen.getByText("(2)")).toBeInTheDocument();
  });

  it("shows a health chip per non-zero health variant in the group", () => {
    render(<RoutineTable {...baseProps({ routines, groupBy: "folder" })} />);
    const backendHeader = screen.getByText("backend/db").closest(".group-hd-row") as HTMLElement;
    // codex/vacuum is disabled, claude/reindex has no registered agent — two distinct health
    // chips, not merged into one.
    expect(within(backendHeader).getAllByText("1")).toHaveLength(2);
  });

  it("collapsing a group hides its rows but keeps the header", () => {
    render(<RoutineTable {...baseProps({ routines, groupBy: "folder" })} />);
    expect(screen.getByText("vacuum")).toBeInTheDocument();
    fireEvent.click(screen.getByText("backend/db"));
    expect(screen.getByText("backend/db")).toBeInTheDocument();
    expect(screen.queryByText("vacuum")).not.toBeInTheDocument();
    // Expanding again brings the rows back.
    fireEvent.click(screen.getByText("backend/db"));
    expect(screen.getByText("vacuum")).toBeInTheDocument();
  });

  it("collapse state survives a remount (persisted to localStorage)", () => {
    const { unmount } = render(<RoutineTable {...baseProps({ routines, groupBy: "folder" })} />);
    fireEvent.click(screen.getByText("backend/db"));
    unmount();
    render(<RoutineTable {...baseProps({ routines, groupBy: "folder" })} />);
    expect(screen.queryByText("vacuum")).not.toBeInTheDocument();
  });

  it("COLLAPSE ALL hides every group's rows, EXPAND ALL restores them", () => {
    render(<RoutineTable {...baseProps({ routines, groupBy: "folder" })} />);
    fireEvent.click(screen.getByText("COLLAPSE ALL"));
    expect(screen.queryByText("Nightly backup")).not.toBeInTheDocument();
    expect(screen.queryByText("vacuum")).not.toBeInTheDocument();
    fireEvent.click(screen.getByText("EXPAND ALL"));
    expect(screen.getByText("Nightly backup")).toBeInTheDocument();
    expect(screen.getByText("vacuum")).toBeInTheDocument();
  });

  it("hides the collapse-all/expand-all controls for a single group", () => {
    render(<RoutineTable {...baseProps({ routines: [routines[0] as RoutineResponse], groupBy: "folder" })} />);
    expect(screen.queryByText("COLLAPSE ALL")).not.toBeInTheDocument();
  });
});

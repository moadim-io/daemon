import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import { RoutineFilesystemTree } from "./RoutineFilesystemTree";

function routine(id: string, title: string, overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id,
    title,
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
    slug: id,
    rel_path: id,
    schedule_description: "hourly",
    goal: null,
    flag_count: 0,
    env_keys: [],
    ...overrides,
  };
}

const callbacks = () => ({
  onEdit: vi.fn(),
  onClone: vi.fn(),
  onDelete: vi.fn(),
  onMove: vi.fn(),
  onToggle: vi.fn(),
  onTrigger: vi.fn(),
  onLogs: vi.fn(),
  onHistory: vi.fn(),
  onFlags: vi.fn(),
  onClearFilters: vi.fn(),
});

describe("RoutineFilesystemTree", () => {
  it("renders routines as nested filesystem folders using folder and slug metadata", () => {
    const routines = [
      routine("root", "Root sync", { slug: "root-sync", rel_path: "root-sync" }),
      routine("db", "DB backup", { folder: "maintenance/db", slug: "backup", rel_path: "maintenance/db/backup" }),
      routine("idx", "Reindex", { folder: "maintenance/search", slug: "reindex", rel_path: "maintenance/search/reindex" }),
    ];

    render(<RoutineFilesystemTree routines={routines} loading={false} filterActive={false} now={new Date(2026, 0, 1)} {...callbacks()} />);

    expect(screen.getByRole("tree", { name: "Routine filesystem" })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /maintenance folder/i })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /db folder/i })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /search folder/i })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /root-sync routine/i })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /maintenance\/db\/backup routine/i })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /maintenance\/search\/reindex routine/i })).toBeInTheDocument();
  });

  it("collapses folders while keeping sibling routines visible", () => {
    const routines = [
      routine("root", "Root sync", { slug: "root-sync", rel_path: "root-sync" }),
      routine("db", "DB backup", { folder: "maintenance/db", slug: "backup", rel_path: "maintenance/db/backup" }),
    ];

    render(<RoutineFilesystemTree routines={routines} loading={false} filterActive={false} now={new Date(2026, 0, 1)} {...callbacks()} />);

    fireEvent.click(screen.getByRole("button", { name: /collapse maintenance folder/i }));

    expect(screen.getByRole("treeitem", { name: /root-sync routine/i })).toBeInTheDocument();
    expect(screen.queryByRole("treeitem", { name: /maintenance\/db\/backup routine/i })).not.toBeInTheDocument();
  });

  it("surfaces the same row actions from each routine node", () => {
    const onMove = vi.fn();
    const onTrigger = vi.fn();
    const onLogs = vi.fn();
    const routineNode = routine("db", "DB backup", { folder: "maintenance/db", slug: "backup", rel_path: "maintenance/db/backup" });

    render(
      <RoutineFilesystemTree
        routines={[routineNode]}
        loading={false}
        filterActive={false}
        now={new Date(2026, 0, 1)}
        {...callbacks()}
        onMove={onMove}
        onTrigger={onTrigger}
        onLogs={onLogs}
      />,
    );

    const node = screen.getByRole("treeitem", { name: /maintenance\/db\/backup routine/i });
    fireEvent.click(within(node).getByRole("button", { name: "Run now" }));
    fireEvent.click(within(node).getByRole("button", { name: "Logs" }));
    fireEvent.click(within(node).getByRole("button", { name: "Move folder" }));

    expect(onTrigger).toHaveBeenCalledWith("db");
    expect(onLogs).toHaveBeenCalledWith("db");
    expect(onMove).toHaveBeenCalledWith("db");
  });
});

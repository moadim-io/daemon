import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { HealthResponse } from "../api/hooks";
import { Header } from "./Header";

function health(overrides: Partial<HealthResponse> = {}): HealthResponse {
  return {
    status: "ok",
    uptime_secs: 42,
    running: true,
    machine: "box-1",
    dependencies: { tmux: true, python3: true },
    crontab_sync: { ok: true },
    version: "3.2.0",
    git_sha: "abc1234",
    build_date: "2026-07-31",
    server_root: "/Users/ofek/.hermes",
    server_exe_dir: "/Users/ofek/.cargo/bin",
    ...overrides,
  };
}

function renderHeader(healthData: HealthResponse) {
  render(
    <Header
      health={healthData}
      healthOk={healthData.running}
      machineName="box-1"
      onRefresh={vi.fn()}
      onStop={vi.fn()}
      onPalette={vi.fn()}
      onRenameMachine={vi.fn()}
      onShortcuts={vi.fn()}
      notifications={{ entries: [], onMarkAllRead: vi.fn(), onClear: vi.fn() }}
    />,
  );
}

describe("Header", () => {
  it("does not warn when crontab sync is healthy", () => {
    renderHeader(health());

    expect(screen.queryByText("⚠ CRON STALE")).not.toBeInTheDocument();
  });

  it("surfaces crontab sync failures in the global header", () => {
    renderHeader(
      health({
        crontab_sync: {
          ok: false,
          last_error: "crontab: crontab - timed out after 15s",
          last_error_at: 1_785_000_000,
        },
      }),
    );

    const warning = screen.getByText("⚠ CRON STALE");
    expect(warning).toBeInTheDocument();
    expect(warning).toHaveAttribute(
      "title",
      "OS crontab sync failed: crontab: crontab - timed out after 15s",
    );
  });
});

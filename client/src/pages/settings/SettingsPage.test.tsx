import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { HealthResponse, MaxConcurrentRunsResponse } from "../../api/hooks";
import { ToastProvider } from "../../shell/toasts";
import { SettingsPage } from "./SettingsPage";

function health(overrides: Partial<HealthResponse> = {}): HealthResponse {
  return {
    status: "ok",
    uptime_secs: 42,
    running: true,
    machine: "box-1",
    dependencies: { tmux: true, python3: true },
    crontab_sync: { ok: true },
    scheduler_resync: { ok: true },
    version: "3.2.0",
    git_sha: "abc1234",
    build_date: "2026-07-31",
    server_root: "/Users/ofek/.hermes",
    server_exe_dir: "/Users/ofek/.cargo/bin",
    ...overrides,
  };
}

function renderPage(
  seedPrompt?: string,
  healthData = health(),
  maxConcurrentRuns: MaxConcurrentRunsResponse = { value: 4, override_value: null },
) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (seedPrompt !== undefined) {
    queryClient.setQueryData(["config", "user-prompt"], seedPrompt);
  }
  queryClient.setQueryData(["health"], healthData);
  queryClient.setQueryData(["config", "max-concurrent-runs"], maxConcurrentRuns);
  render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <SettingsPage />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

describe("SettingsPage", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("shows a loading state before the prompt loads", () => {
    renderPage();
    expect(screen.getByText("Loading…")).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/always run/)).not.toBeInTheDocument();
  });

  it("moves the data refresh cadence control into settings and persists changes", () => {
    localStorage.setItem("moadim.refresh-interval", "15");
    renderPage("existing prompt");

    const select = screen.getByRole("combobox", { name: "Auto-refresh interval" });
    expect(select).toHaveValue("15");

    fireEvent.change(select, { target: { value: "30" } });

    expect(select).toHaveValue("30");
    expect(localStorage.getItem("moadim.refresh-interval")).toBe("30");
  });

  it("moves the light/dark theme control into settings and persists changes", () => {
    localStorage.setItem("moadim.client.theme", "light");
    renderPage("existing prompt");

    const light = screen.getByRole("button", { name: "Light" });
    const dark = screen.getByRole("button", { name: "Dark" });
    expect(light).toHaveAttribute("aria-pressed", "true");
    expect(dark).toHaveAttribute("aria-pressed", "false");

    fireEvent.click(dark);

    expect(localStorage.getItem("moadim.client.theme")).toBe("dark");
    expect(light).toHaveAttribute("aria-pressed", "false");
    expect(dark).toHaveAttribute("aria-pressed", "true");
  });


  it("shows crontab sync recovery guidance when system health is stale", () => {
    renderPage(
      "existing prompt",
      health({
        crontab_sync: {
          ok: false,
          last_error: "crontab: crontab - timed out after 15s",
          last_error_at: 1_785_000_000,
        },
      }),
    );

    expect(screen.getByText("System health")).toBeInTheDocument();
    expect(screen.getByText("⚠ OS crontab sync needs attention")).toBeInTheDocument();
    expect(screen.getByText("crontab: crontab - timed out after 15s")).toBeInTheDocument();
    expect(screen.getByText(/Full Disk Access/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry sync now" })).toBeInTheDocument();
  });

  function promptSaveButton() {
    const saveButtons = screen.getAllByRole("button", { name: "Save" });
    return saveButtons[saveButtons.length - 1];
  }

  it("seeds the textarea from the loaded prompt and disables save until edited", () => {
    renderPage("existing prompt");
    expect(screen.getByPlaceholderText(/always run/)).toHaveValue("existing prompt");
    expect(promptSaveButton()).toBeDisabled();
    expect(screen.queryByText("unsaved changes")).not.toBeInTheDocument();
  });

  it("marks the draft dirty and enables save once edited", () => {
    renderPage("existing prompt");
    fireEvent.change(screen.getByPlaceholderText(/always run/), {
      target: { value: "existing prompt, edited" },
    });
    expect(promptSaveButton()).not.toBeDisabled();
    expect(screen.getByText("unsaved changes")).toBeInTheDocument();
  });

  describe("concurrency cap", () => {
    it("shows the effective cap and seeds the input from the saved override", () => {
      renderPage("existing prompt", health(), { value: 6, override_value: 6 });
      expect(screen.getByText("6", { selector: "strong" })).toBeInTheDocument();
      expect(screen.getByLabelText("Max concurrent runs override")).toHaveValue(6);
    });

    it("shows unbounded when the effective cap is 0", () => {
      renderPage("existing prompt", health(), { value: 0, override_value: null });
      expect(screen.getByText("unbounded")).toBeInTheDocument();
    });

    it("leaves the override input blank when no override is saved", () => {
      renderPage("existing prompt", health(), { value: 4, override_value: null });
      expect(screen.getByLabelText("Max concurrent runs override")).toHaveValue(null);
    });

    it("disables save until the override is edited, and rejects non-numeric input", () => {
      renderPage("existing prompt", health(), { value: 4, override_value: null });
      const saveButtons = screen.getAllByRole("button", { name: "Save" });
      const concurrencySave = saveButtons[0];
      expect(concurrencySave).toBeDisabled();

      const input = screen.getByLabelText("Max concurrent runs override");
      fireEvent.change(input, { target: { value: "-1" } });
      expect(screen.getByText("must be a whole number ≥ 0")).toBeInTheDocument();
      expect(concurrencySave).toBeDisabled();

      fireEvent.change(input, { target: { value: "8" } });
      expect(concurrencySave).not.toBeDisabled();
      expect(screen.getByText("unsaved changes")).toBeInTheDocument();
    });

    it("only shows Clear once an override value is entered", () => {
      renderPage("existing prompt", health(), { value: 4, override_value: null });
      expect(screen.queryByRole("button", { name: "Clear" })).not.toBeInTheDocument();

      fireEvent.change(screen.getByLabelText("Max concurrent runs override"), { target: { value: "8" } });
      expect(screen.getByRole("button", { name: "Clear" })).toBeInTheDocument();

      fireEvent.click(screen.getByRole("button", { name: "Clear" }));
      expect(screen.getByLabelText("Max concurrent runs override")).toHaveValue(null);
      expect(screen.queryByRole("button", { name: "Clear" })).not.toBeInTheDocument();
    });
  });
});

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { ToastProvider } from "../../shell/toasts";
import { SettingsPage } from "./SettingsPage";

function renderPage(seedPrompt?: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (seedPrompt !== undefined) {
    queryClient.setQueryData(["config", "user-prompt"], seedPrompt);
  }
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

  it("seeds the textarea from the loaded prompt and disables save until edited", () => {
    renderPage("existing prompt");
    expect(screen.getByPlaceholderText(/always run/)).toHaveValue("existing prompt");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(screen.queryByText("unsaved changes")).not.toBeInTheDocument();
  });

  it("marks the draft dirty and enables save once edited", () => {
    renderPage("existing prompt");
    fireEvent.change(screen.getByPlaceholderText(/always run/), {
      target: { value: "existing prompt, edited" },
    });
    expect(screen.getByRole("button", { name: "Save" })).not.toBeDisabled();
    expect(screen.getByText("unsaved changes")).toBeInTheDocument();
  });
});

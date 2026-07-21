import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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
  it("shows a loading state before the prompt loads", () => {
    renderPage();
    expect(screen.getByText("Loading…")).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/always run/)).not.toBeInTheDocument();
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

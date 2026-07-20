import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RoutineForm } from "./RoutineForm";

function renderForm(props: Partial<React.ComponentProps<typeof RoutineForm>> = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const onSave = vi.fn();
  const onCancel = vi.fn();
  render(
    <QueryClientProvider client={queryClient}>
      <RoutineForm mode="create" saving={false} onCancel={onCancel} onSave={onSave} {...props} />
    </QueryClientProvider>,
  );
  return { onSave, onCancel };
}

describe("RoutineForm validation", () => {
  it("disables save until title, schedule, agent, and prompt are all non-blank", () => {
    renderForm();
    const save = screen.getByRole("button", { name: "CREATE ROUTINE" });
    expect(save).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("ops/nightly triage"), { target: { value: "My routine" } });
    expect(save).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("sec min hour dom month dow year"), {
      target: { value: "@daily" },
    });
    // Agent already defaults to "claude" — still missing prompt.
    expect(save).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("Review open PRs and summarize…"), {
      target: { value: "Do the thing" },
    });
    expect(save).not.toBeDisabled();
  });

  it("whitespace-only fields do not count as filled", () => {
    renderForm();
    fireEvent.change(screen.getByPlaceholderText("ops/nightly triage"), { target: { value: "   " } });
    fireEvent.change(screen.getByPlaceholderText("sec min hour dom month dow year"), {
      target: { value: "@daily" },
    });
    fireEvent.change(screen.getByPlaceholderText("Review open PRs and summarize…"), {
      target: { value: "Do the thing" },
    });
    expect(screen.getByRole("button", { name: "CREATE ROUTINE" })).toBeDisabled();
  });

  it("submits the parsed draft on save", async () => {
    const { onSave } = renderForm();
    fireEvent.change(screen.getByPlaceholderText("ops/nightly triage"), { target: { value: "My routine" } });
    fireEvent.change(screen.getByPlaceholderText("sec min hour dom month dow year"), {
      target: { value: "@daily" },
    });
    fireEvent.change(screen.getByPlaceholderText("Review open PRs and summarize…"), {
      target: { value: "Do the thing" },
    });
    fireEvent.click(screen.getByRole("button", { name: "CREATE ROUTINE" }));
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({ title: "My routine", schedule: "@daily", prompt: "Do the thing", agent: "claude" }),
      ),
    );
  });

  it("a cron preset button fills the schedule field", () => {
    renderForm();
    fireEvent.click(screen.getByRole("button", { name: "every hour" }));
    expect(screen.getByPlaceholderText("sec min hour dom month dow year")).toHaveValue("0 0 * * * * *");
  });

  it("edit mode renders the modal chrome with SAVE CHANGES", () => {
    renderForm({ mode: "edit" });
    expect(screen.getByText("EDIT ROUTINE")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "SAVE CHANGES" })).toBeInTheDocument();
  });
});

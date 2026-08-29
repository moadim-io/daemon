import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RoutineForm, updateRequestFromDraft, type RoutineDraft } from "./RoutineForm";

function routineDraft(): RoutineDraft {
  return {
    schedule: "@daily",
    schedules: ["@daily", "@hourly"],
    title: "My routine",
    agent: "claude",
    model: null,
    prompt: "Do the thing",
    goal: null,
    repositories: [],
    machines: [],
    enabled: true,
    power_saving_exempt: false,
    ttl_secs: null,
    tags: [],
  };
}

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

const firstCronExpression = () => screen.getByRole("textbox", { name: "Cron expression 1" });

describe("RoutineForm validation", () => {
  it("sends schedules, not the legacy schedule field, when editing", () => {
    const request = updateRequestFromDraft(routineDraft());

    expect(request).not.toHaveProperty("schedule");
    expect(request).toMatchObject({ schedules: ["@daily", "@hourly"], title: "My routine" });
  });

  it("disables save until title, schedule, agent, and prompt are all non-blank", () => {
    renderForm();
    const save = screen.getByRole("button", { name: "CREATE ROUTINE" });
    expect(save).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("ops/nightly triage"), { target: { value: "My routine" } });
    expect(save).toBeDisabled();

    fireEvent.change(firstCronExpression(), { target: { value: "@daily" } });
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
    fireEvent.change(firstCronExpression(), { target: { value: "@daily" } });
    fireEvent.change(screen.getByPlaceholderText("Review open PRs and summarize…"), {
      target: { value: "Do the thing" },
    });
    expect(screen.getByRole("button", { name: "CREATE ROUTINE" })).toBeDisabled();
  });

  it("submits every cron expression as the canonical schedules array", async () => {
    const { onSave } = renderForm();
    fireEvent.change(screen.getByPlaceholderText("ops/nightly triage"), { target: { value: "My routine" } });
    fireEvent.change(firstCronExpression(), { target: { value: "@daily" } });
    fireEvent.click(screen.getByRole("button", { name: "+ ADD CRON EXPRESSION" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Cron expression 2" }), { target: { value: "@hourly" } });
    fireEvent.change(screen.getByPlaceholderText("Review open PRs and summarize…"), {
      target: { value: "Do the thing" },
    });
    fireEvent.click(screen.getByRole("button", { name: "CREATE ROUTINE" }));
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "My routine",
          schedule: "@daily",
          schedules: ["@daily", "@hourly"],
          prompt: "Do the thing",
          agent: "claude",
        }),
      ),
    );
  });

  it("adds and removes independent cron expressions", () => {
    renderForm();

    fireEvent.change(firstCronExpression(), { target: { value: "@daily" } });
    fireEvent.click(screen.getByRole("button", { name: "+ ADD CRON EXPRESSION" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Cron expression 2" }), { target: { value: "@hourly" } });

    expect(firstCronExpression()).toHaveValue("@daily");
    expect(screen.getByRole("textbox", { name: "Cron expression 2" })).toHaveValue("@hourly");

    fireEvent.click(screen.getByRole("button", { name: "Remove cron expression 1" }));
    expect(firstCronExpression()).toHaveValue("@hourly");
  });

  it("a cron preset fills an empty expression and appends to an existing one", () => {
    renderForm();
    fireEvent.click(screen.getByRole("button", { name: "every hour" }));
    expect(firstCronExpression()).toHaveValue("0 0 * * * * *");

    fireEvent.click(screen.getByRole("button", { name: "@daily" }));
    expect(screen.getByRole("textbox", { name: "Cron expression 2" })).toHaveValue("@daily");
  });

  it("edit mode renders the modal chrome with SAVE CHANGES", () => {
    renderForm({ mode: "edit" });
    expect(screen.getByText("EDIT ROUTINE")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "SAVE CHANGES" })).toBeInTheDocument();
  });
});

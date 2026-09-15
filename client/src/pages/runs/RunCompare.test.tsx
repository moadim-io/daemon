import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { RunSummary } from "../../api/hooks";
import { RunCompare } from "./RunCompare";

function run(overrides: Partial<RunSummary> = {}): RunSummary {
  return {
    workbench: "job-2000",
    started_at: 2_000,
    started_at_local: "",
    finished_at: 2_060,
    finished_at_local: "",
    status: "failed",
    exit_code: 1,
    retention_expires_at: 90_000,
    ...overrides,
  };
}

function renderCompare(current: RunSummary, candidates: RunSummary[], logs: Record<string, string>) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  for (const [workbench, text] of Object.entries(logs)) {
    queryClient.setQueryData(["routines", "r1", "runs", workbench, "log"], text);
  }
  render(
    <QueryClientProvider client={queryClient}>
      <RunCompare routineId="r1" current={current} candidates={candidates} />
    </QueryClientProvider>,
  );
}

describe("RunCompare", () => {
  it("renders nothing when there are no other runs to compare against", () => {
    const { container } = render(
      <QueryClientProvider client={new QueryClient()}>
        <RunCompare routineId="r1" current={run()} candidates={[]} />
      </QueryClientProvider>,
    );
    expect(container.firstChild).toBeNull();
  });

  it("stays collapsed until the toggle is clicked", () => {
    const older = run({ workbench: "job-1000", started_at: 1_000, status: "success" });
    renderCompare(run(), [older], {});
    expect(screen.queryByLabelText("Compare against")).not.toBeInTheDocument();
  });

  it("defaults the comparison target to the last success before the current run", () => {
    const olderFail = run({ workbench: "job-500", started_at: 500, status: "failed" });
    const lastSuccess = run({ workbench: "job-1000", started_at: 1_000, status: "success" });
    const current = run({ workbench: "job-2000", started_at: 2_000, status: "failed" });
    renderCompare(current, [lastSuccess, olderFail], {
      "job-2000": "a\nb\n",
      "job-1000": "a\nc\n",
    });
    fireEvent.click(screen.getByText(/Compare against another run/));
    const select = screen.getByLabelText("Compare against") as HTMLSelectElement;
    expect(select.value).toBe("job-1000");
  });

  it("shows a line-diff summary against the selected run", async () => {
    const target = run({ workbench: "job-1000", started_at: 1_000, status: "success" });
    const current = run({ workbench: "job-2000", started_at: 2_000, status: "failed" });
    renderCompare(current, [target], {
      "job-2000": "a\nb\n",
      "job-1000": "a\nc\n",
    });
    fireEvent.click(screen.getByText(/Compare against another run/));
    expect(await screen.findByText("-1", { exact: false })).toBeInTheDocument();
    expect(screen.getByText("+1", { exact: false })).toBeInTheDocument();
  });
});

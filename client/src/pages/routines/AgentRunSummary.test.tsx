import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AgentRunSummary } from "./AgentRunSummary";

describe("AgentRunSummary", () => {
  it("renders nothing while the query hasn't resolved and isn't loading", () => {
    const { container } = render(<AgentRunSummary content={undefined} loading={false} err={undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a spinner while loading", () => {
    render(<AgentRunSummary content={undefined} loading={true} err={undefined} />);
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows the error message on failure", () => {
    render(<AgentRunSummary content={undefined} loading={false} err="boom" />);
    expect(screen.getByText("Error: boom")).toBeInTheDocument();
  });

  it("shows an empty-state note when the agent wrote no summary", () => {
    render(<AgentRunSummary content="" loading={false} err={undefined} />);
    expect(screen.getByText("— the agent didn't write a summary for this run —")).toBeInTheDocument();
  });

  it("renders the summary text preserving newlines", () => {
    render(<AgentRunSummary content={"fixed the flaky test\n\nsee logs for detail"} loading={false} err={undefined} />);
    expect(screen.getByText(/fixed the flaky test/)).toBeInTheDocument();
    expect(screen.getByText(/see logs for detail/)).toBeInTheDocument();
  });
});

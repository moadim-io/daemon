import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ViewToggle, type RView } from "./ViewToggle";

function renderToggle(view: RView) {
  const onSetView = vi.fn();
  render(<ViewToggle view={view} onSetView={onSetView} />);
  return onSetView;
}

describe("ViewToggle", () => {
  it("renders a button for each view with its label", () => {
    renderToggle("table");
    expect(screen.getByText("LIST")).toBeInTheDocument();
    expect(screen.getByText("CALENDAR")).toBeInTheDocument();
    expect(screen.getByText("DAY")).toBeInTheDocument();
  });

  it("marks only the current view's button active", () => {
    renderToggle("calendar");
    expect(screen.getByText("LIST")).not.toHaveClass("active");
    expect(screen.getByText("CALENDAR")).toHaveClass("active");
    expect(screen.getByText("DAY")).not.toHaveClass("active");
  });

  it("calls onSetView with the clicked view", () => {
    const onSetView = renderToggle("table");
    fireEvent.click(screen.getByText("DAY"));
    expect(onSetView).toHaveBeenCalledWith("day");
  });

  it("still calls onSetView when clicking the already-active view", () => {
    // Unlike StatsBar's toggle-off facets, ViewToggle has no "none" state — re-clicking
    // the active view is a no-op for the caller to handle, not something this component
    // should suppress.
    const onSetView = renderToggle("table");
    fireEvent.click(screen.getByText("LIST"));
    expect(onSetView).toHaveBeenCalledWith("table");
  });
});

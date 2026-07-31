import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DayTimeline, type TimelineItem } from "./DayTimeline";

const item: TimelineItem = {
  id: "daily-digest",
  label: "Daily digest",
  schedule: "0 * * * *",
  snoozed: false,
  flagCount: 0,
};

describe("DayTimeline", () => {
  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
  });

  it("renders timeline routine chips as edit buttons when a click handler is provided", () => {
    const onClick = vi.fn();

    render(<DayTimeline items={[item]} loading={false} onClick={onClick} />);

    const editButtons = screen.getAllByRole("button", { name: "Edit Daily digest" });
    expect(editButtons.length).toBeGreaterThan(0);

    fireEvent.click(editButtons[0] as HTMLElement);
    expect(onClick).toHaveBeenCalledWith("daily-digest");
  });

  it("keeps non-clickable entries as text when no click handler is provided", () => {
    const { container } = render(<DayTimeline items={[item]} loading={false} />);

    expect(screen.queryByRole("button", { name: "Edit Daily digest" })).not.toBeInTheDocument();
    expect(container.querySelectorAll(".day-chip").length).toBeGreaterThan(0);
  });
});

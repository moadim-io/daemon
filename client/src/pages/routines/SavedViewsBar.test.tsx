import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SavedViewsBar } from "./SavedViewsBar";
import type { SavedView, ViewSnapshot } from "./savedViews";

const addToast = vi.fn();
vi.mock("../../shell/toasts", () => ({ useToasts: () => ({ addToast }) }));

const snapshot: ViewSnapshot = {
  query: "",
  status: "all",
  agent: " all",
  machine: " any",
  repository: " all",
  tag: " all",
  sortCol: undefined,
  sortDir: "asc",
  groupBy: "none",
};

function renderBar(views: SavedView[] = []) {
  const handlers = { onApply: vi.fn(), onSave: vi.fn(), onDelete: vi.fn() };
  render(<SavedViewsBar views={views} {...handlers} />);
  return handlers;
}

describe("SavedViewsBar", () => {
  beforeEach(() => {
    addToast.mockClear();
    vi.stubGlobal("navigator", { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it("applies the picked saved view", () => {
    const handlers = renderBar([{ name: "My View", snapshot }]);
    fireEvent.change(screen.getByLabelText("Saved views"), { target: { value: "My View" } });
    expect(handlers.onApply).toHaveBeenCalledWith(snapshot);
  });

  it("saves a new view with the typed name", () => {
    const handlers = renderBar();
    fireEvent.click(screen.getByText("☆ SAVE VIEW"));
    fireEvent.change(screen.getByLabelText("New view name"), { target: { value: "Attention" } });
    fireEvent.click(screen.getByText("SAVE"));
    expect(handlers.onSave).toHaveBeenCalledWith("Attention");
  });

  it("copies the current location to the clipboard", async () => {
    renderBar();
    fireEvent.click(screen.getByText("🔗 COPY LINK"));
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(window.location.href);
    await vi.waitFor(() => expect(addToast).toHaveBeenCalledWith(expect.stringMatching(/Link copied/), "ok"));
  });

  it("shows an error toast when the clipboard write fails", async () => {
    vi.stubGlobal("navigator", { clipboard: { writeText: vi.fn().mockRejectedValue(new Error("denied")) } });
    renderBar();
    fireEvent.click(screen.getByText("🔗 COPY LINK"));
    await vi.waitFor(() => expect(addToast).toHaveBeenCalledWith("Copy failed", "err"));
  });
});

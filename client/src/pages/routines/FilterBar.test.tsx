import { createRef } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { defaultRoutineFilter } from "./filter";
import { FilterBar } from "./FilterBar";

function renderBar(overrides: Partial<Parameters<typeof FilterBar>[0]> = {}) {
  const searchRef = createRef<HTMLInputElement>();
  const handlers = {
    onQuery: vi.fn(),
    onStatus: vi.fn(),
    onAgent: vi.fn(),
    onMachine: vi.fn(),
    onRepository: vi.fn(),
    onTag: vi.fn(),
    onClear: vi.fn(),
  };
  render(
    <FilterBar
      filter={defaultRoutineFilter()}
      agents={["claude", "codex"]}
      machines={["box-1"]}
      repositories={["org/repo"]}
      tags={[]}
      shown={3}
      total={5}
      searchRef={searchRef}
      {...handlers}
      {...overrides}
    />,
  );
  return handlers;
}

describe("FilterBar", () => {
  it("shows the shown/total count", () => {
    renderBar();
    expect(screen.getByText("Showing 3 of 5")).toBeInTheDocument();
  });

  it("typing in the search box calls onQuery", () => {
    const handlers = renderBar();
    fireEvent.change(screen.getByPlaceholderText(/Search routines/), { target: { value: "deploy" } });
    expect(handlers.onQuery).toHaveBeenCalledWith("deploy");
  });

  it("changing the status select calls onStatus with the parsed facet", () => {
    const handlers = renderBar();
    fireEvent.change(screen.getByLabelText("Status filter"), { target: { value: "dormant" } });
    expect(handlers.onStatus).toHaveBeenCalledWith("dormant");
  });

  it("hides the CLEAR button when no filter is active", () => {
    renderBar();
    expect(screen.queryByRole("button", { name: "CLEAR" })).not.toBeInTheDocument();
  });

  it("shows and wires the CLEAR button when a filter is active", () => {
    const handlers = renderBar({ filter: { ...defaultRoutineFilter(), query: "x" } });
    const clearBtn = screen.getByRole("button", { name: "CLEAR" });
    fireEvent.click(clearBtn);
    expect(handlers.onClear).toHaveBeenCalled();
  });

  it("omits the tag select when there are no tags", () => {
    renderBar({ tags: [] });
    expect(screen.queryByLabelText("Tag filter")).not.toBeInTheDocument();
  });

  it("shows the tag select when tags exist", () => {
    renderBar({ tags: ["nightly"] });
    expect(screen.getByLabelText("Tag filter")).toBeInTheDocument();
  });
});

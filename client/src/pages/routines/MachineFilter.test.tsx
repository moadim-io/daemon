import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it } from "vitest";
import { MachineFilter } from "./MachineFilter";
import type { RoutineMachineFacet } from "./filter";

it("toggles multiple machines, combines None, and resets to Any", () => {
  function Harness() {
    const [facet, setFacet] = useState<RoutineMachineFacet>({ kind: "any" });
    return <MachineFilter facet={facet} machines={["a", "b"]} onChange={setFacet} />;
  }
  render(<Harness />);
  fireEvent.click(screen.getByLabelText("Machine filter"));
  fireEvent.click(screen.getByLabelText("a"));
  fireEvent.click(screen.getByLabelText("b"));
  expect(screen.getByLabelText("Machine filter")).toHaveTextContent("2 selected");
  fireEvent.click(screen.getByLabelText("None (unassigned)"));
  expect(screen.getByLabelText("Machine filter")).toHaveTextContent("3 selected");
  fireEvent.click(screen.getByLabelText("a"));
  expect(screen.getByLabelText("a")).not.toBeChecked();
  expect(screen.getByLabelText("b")).toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Any machine" }));
  expect(screen.getByLabelText("Machine filter")).toHaveTextContent("Any");
  expect(screen.getByLabelText("b")).not.toBeChecked();
});


it("keeps absent machines from old views removable and distinguishes special names", () => {
  function Harness() {
    const [facet, setFacet] = useState<RoutineMachineFacet>({ kind: "machine", value: "old,雪" });
    return <MachineFilter facet={facet} machines={["None", "any"]} onChange={setFacet} />;
  }
  render(<Harness />);
  fireEvent.click(screen.getByLabelText("Machine filter"));
  expect(screen.getByLabelText("old,雪")).toBeChecked();
  fireEvent.click(screen.getByLabelText("None"));
  expect(screen.getByLabelText("None (unassigned)")).not.toBeChecked();
  fireEvent.click(screen.getByLabelText("old,雪"));
  expect(screen.getByLabelText("Machine filter")).toHaveTextContent("None");
  fireEvent.click(screen.getByLabelText("None"));
  expect(screen.getByLabelText("Machine filter")).toHaveTextContent("Any");
});

it("dismisses on Escape with summary focus, outside pointers, and focus leaving", async () => {
  render(<><MachineFilter facet={{ kind: "any" }} machines={["a", "b"]} onChange={() => {}} /><button>Outside</button></>);
  const summary = screen.getByLabelText("Machine filter");
  const details = summary.closest("details")!;
  const a = screen.getByLabelText("a");
  const outside = screen.getByRole("button", { name: "Outside" });
  fireEvent.click(summary);
  a.focus();
  fireEvent.keyDown(a, { key: "Escape" });
  expect(details.open).toBe(false);
  expect(summary).toHaveFocus();
  for (const pointerType of ["mouse", "touch", "pen"]) {
    fireEvent.click(summary);
    fireEvent.pointerDown(a, { pointerType });
    expect(details.open).toBe(true);
    fireEvent.pointerDown(outside, { pointerType });
    expect(details.open).toBe(false);
  }
  fireEvent.click(summary);
  a.focus();
  screen.getByLabelText("b").focus();
  expect(details.open).toBe(true);
  outside.focus();
  await waitFor(() => expect(details.open).toBe(false));
  expect(outside).toHaveFocus();
  fireEvent.click(summary);
  a.focus();
  a.blur();
  await waitFor(() => expect(details.open).toBe(false));
});

it("waits for label focus to settle before deciding whether to dismiss", async () => {
  render(<MachineFilter facet={{ kind: "any" }} machines={["a"]} onChange={() => {}} />);
  const summary = screen.getByLabelText("Machine filter");
  const details = summary.closest("details")!;
  fireEvent.click(summary);
  summary.focus();
  summary.blur();
  expect(details.open).toBe(true);
  const checkbox = screen.getByLabelText("a");
  checkbox.focus();
  await new Promise((resolve) => setTimeout(resolve, 10));
  expect(details.open).toBe(true);
  expect(checkbox).toHaveFocus();
});

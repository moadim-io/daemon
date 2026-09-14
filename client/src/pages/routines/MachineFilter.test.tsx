import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
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

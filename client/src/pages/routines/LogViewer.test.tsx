import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { LogViewer } from "./LogViewer";

const LOG = ["starting", "WARN slow disk", "ok", "ERROR boom", "done", "error again"].join("\n");

function lineNumbers(): string[] {
  return Array.from(document.querySelectorAll(".log-line")).map((el) => el.getAttribute("data-ln") ?? "");
}

describe("LogViewer", () => {
  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
  });

  it("shows a spinner while loading", () => {
    const { container } = render(<LogViewer content={undefined} loading err={undefined} />);
    expect(container.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows an error", () => {
    render(<LogViewer content={undefined} loading={false} err="nope" />);
    expect(screen.getByText("Error: nope")).toBeInTheDocument();
  });

  it("shows the empty state", () => {
    render(<LogViewer content="" loading={false} err={undefined} />);
    expect(screen.getByText("— no logs yet —")).toBeInTheDocument();
  });

  it("renders nothing without content", () => {
    const { container } = render(<LogViewer content={undefined} loading={false} err={undefined} />);
    expect(container.querySelector(".logs-wrap")).toBeEmptyDOMElement();
  });

  it("counts and color-codes error and warning lines", () => {
    render(<LogViewer content={LOG} loading={false} err={undefined} />);
    expect(screen.getByText("2 errors")).toBeInTheDocument();
    expect(screen.getByText("1 warning")).toBeInTheDocument();
    expect(document.querySelectorAll(".log-sev-error")).toHaveLength(2);
    expect(document.querySelectorAll(".log-sev-warn")).toHaveLength(1);
    expect(screen.getByText("6 lines")).toBeInTheDocument();
  });

  it("dims zero counts and disables stepping when there are no problems", () => {
    render(<LogViewer content={"a\nb"} loading={false} err={undefined} />);
    expect(screen.getByText("0 errors")).toHaveClass("is-zero");
    expect(screen.getByText("0 warnings")).toHaveClass("is-zero");
    expect(screen.getByRole("button", { name: "Next problem" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Previous problem" })).toBeDisabled();
  });

  it("filters to problems only", () => {
    render(<LogViewer content={LOG} loading={false} err={undefined} />);
    fireEvent.click(screen.getByLabelText("Problems only"));
    expect(lineNumbers()).toEqual(["2", "4", "6"]);
  });

  it("steps through problems with wrap-around and scrolls to them", () => {
    render(<LogViewer content={LOG} loading={false} err={undefined} />);
    const next = screen.getByRole("button", { name: "Next problem" });
    const prev = screen.getByRole("button", { name: "Previous problem" });
    const current = () => document.querySelector(".log-line-current")?.getAttribute("data-ln");
    fireEvent.click(next);
    expect(current()).toBe("2");
    fireEvent.click(next);
    expect(current()).toBe("4");
    fireEvent.click(prev);
    expect(current()).toBe("2");
    fireEvent.click(prev);
    expect(current()).toBe("6");
    expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
  });

  it("composes search with highlighting, clearing, and an empty result", () => {
    render(<LogViewer content={LOG} loading={false} err={undefined} />);
    const input = screen.getByLabelText("Search log lines");
    fireEvent.change(input, { target: { value: "error" } });
    expect(lineNumbers()).toEqual(["4", "6"]);
    expect(screen.getByText("2 / 6 matches")).toBeInTheDocument();
    expect(document.querySelectorAll("mark.log-hl")).toHaveLength(2);
    fireEvent.change(input, { target: { value: "zzz" } });
    expect(screen.getByText("— no matching lines —")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(lineNumbers()).toHaveLength(6);
  });
});

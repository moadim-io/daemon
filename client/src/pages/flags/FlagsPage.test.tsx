import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Flag, RoutineResponse } from "../../api/hooks";
import { FlagsPage } from "./FlagsPage";

const resolveMutate = vi.fn();
vi.mock("../../api/hooks", async (importActual) => ({
  ...(await importActual<typeof import("../../api/hooks")>()),
  useResolveFlag: () => ({ mutate: resolveMutate, isPending: false }),
}));
import { buildFlagRows, filterFlagRows, flagStats, flagTypes } from "./flagsInbox";

function routine(id: string, flag_count: number): RoutineResponse {
  return { id, title: `Title ${id}`, flag_count } as RoutineResponse;
}

function flag(filename: string, type: string, created_at: number, description = filename): Flag {
  return { filename, type, created_at, description, scope: "general" };
}

const r1 = routine("r1", 2);
const r2 = routine("r2", 1);
const r3 = routine("r3", 0);
const r1Flags = [flag("b.md", "bug", 200, "crash on start"), flag("q.md", "question", 50, "which repo?")];
const r2Flags = [{ ...flag("g.md", "gap", 100, "no docs"), scope: "local" as const }];

function renderPage(seed: (qc: QueryClient) => void) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  seed(queryClient);
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <FlagsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function seedAll(qc: QueryClient) {
  qc.setQueryData(["routines", {}], [r1, r2, r3]);
  qc.setQueryData(["routines", "r1", "flags"], r1Flags);
  qc.setQueryData(["routines", "r2", "flags"], r2Flags);
}

beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe("flagsInbox", () => {
  const rows = buildFlagRows([r1, r2], [r1Flags, r2Flags]);

  it("joins flags to routines, oldest first, tolerating unloaded lists", () => {
    expect(rows.map((r) => r.flag.filename)).toEqual(["q.md", "g.md", "b.md"]);
    expect(rows[0]?.routineTitle).toBe("Title r1");
    expect(buildFlagRows([r1], [undefined])).toEqual([]);
  });

  it("filters by text and type", () => {
    expect(filterFlagRows(rows, "  CRASH ", "").map((r) => r.flag.filename)).toEqual(["b.md"]);
    expect(filterFlagRows(rows, "title r2", "").map((r) => r.flag.filename)).toEqual(["g.md"]);
    expect(filterFlagRows(rows, "", "question").map((r) => r.flag.filename)).toEqual(["q.md"]);
    expect(filterFlagRows(rows, "crash", "gap")).toEqual([]);
  });

  it("lists distinct types and computes stats", () => {
    expect(flagTypes(rows)).toEqual(["bug", "gap", "question"]);
    expect(flagStats(rows)).toEqual({ open: 3, routines: 2, oldestAt: 50 });
    expect(flagStats([])).toEqual({ open: 0, routines: 0, oldestAt: null });
  });
});

describe("FlagsPage", () => {
  it("shows a spinner while loading", () => {
    vi.spyOn(globalThis, "fetch").mockReturnValue(new Promise(() => {}));
    renderPage(() => {});
    expect(document.querySelector(".spinner")).toBeInTheDocument();
  });

  it("shows an error state", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("boom", { status: 500 }));
    renderPage(() => {});
    expect(await screen.findByText("FAILED TO LOAD")).toBeInTheDocument();
  });

  it("shows inbox zero when no routine is flagged", () => {
    renderPage((qc) => qc.setQueryData(["routines", {}], [r3]));
    expect(screen.getByText("INBOX ZERO")).toBeInTheDocument();
    expect(screen.getByText("OLDEST FLAG").nextElementSibling).toHaveTextContent("—");
  });

  it("lists every open flag oldest first with stats, links, search, type filter and sort", () => {
    renderPage(seedAll);
    expect(screen.getByRole("heading", { name: "Flags" })).toBeInTheDocument();
    expect(screen.getByText("OPEN FLAGS").nextElementSibling).toHaveTextContent("3");
    expect(screen.getByText("ROUTINES FLAGGED").nextElementSibling).toHaveTextContent("2");

    const descs = () => [...document.querySelectorAll("tbody .flag-desc")].map((e) => e.textContent);
    expect(descs()).toEqual(["which repo?", "no docs", "crash on start"]);
    expect(screen.getByRole("link", { name: "Title r2" })).toHaveAttribute("href", "/routines?history=r2");
    expect(screen.getByText("local")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "AGE" }));
    fireEvent.click(screen.getByRole("button", { name: /AGE/ }));
    expect(descs()).toEqual(["crash on start", "no docs", "which repo?"]);
    fireEvent.click(screen.getByRole("button", { name: "TYPE" }));
    expect(descs()).toEqual(["crash on start", "no docs", "which repo?"]);
    fireEvent.click(screen.getByRole("button", { name: "ROUTINE" }));
    expect(descs()).toEqual(["which repo?", "crash on start", "no docs"]);

    fireEvent.change(screen.getByLabelText("Flag type"), { target: { value: "gap" } });
    expect(descs()).toEqual(["no docs"]);
    expect(screen.getByText("1 of 3")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Flag type"), { target: { value: "" } });
    fireEvent.change(screen.getByLabelText("Search flags"), { target: { value: "nothing-matches" } });
    expect(screen.getByText("NO MATCHING FLAGS")).toBeInTheDocument();
  });

  it("resolves a flag inline", () => {
    renderPage(seedAll);
    const row = screen.getByText("no docs").closest("tr")!;
    fireEvent.click(within(row).getByRole("button", { name: "Resolve gap flag on Title r2" }));
    expect(resolveMutate).toHaveBeenCalledWith({ id: "r2", filename: "g.md" });
  });
});

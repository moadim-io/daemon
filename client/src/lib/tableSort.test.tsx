import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FleetRunSummary } from "../api/hooks";
import { ReliabilityPage } from "../pages/reliability/ReliabilityPage";
import { RunsPage } from "../pages/runs/RunsPage";
import { loadSort, nextSort, saveSort, sortRows, type SortAccessors } from "./tableSort";

interface Row {
  name: string;
  n: number | null;
}
const ACC: SortAccessors<Row, "name" | "n"> = { name: (r) => r.name, n: (r) => r.n };
const ROWS: Row[] = [
  { name: "b", n: 2 },
  { name: "a10", n: null },
  { name: "A2", n: 1 },
  { name: "c", n: 2 },
];
const names = (rows: Row[]) => rows.map((r) => r.name);

beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe("nextSort", () => {
  it("starts a new column at its default dir, flips, then clears", () => {
    const first = nextSort<"a" | "b">(null, "a", "desc");
    expect(first).toEqual({ key: "a", dir: "desc" });
    const second = nextSort(first, "a", "desc");
    expect(second).toEqual({ key: "a", dir: "asc" });
    expect(nextSort(second, "a", "desc")).toBeNull();
    expect(nextSort(second, "b", "asc")).toEqual({ key: "b", dir: "asc" });
    expect(nextSort({ key: "a", dir: "asc" }, "a", "asc")).toEqual({ key: "a", dir: "desc" });
  });
});

describe("sortRows", () => {
  it("returns a copy in natural order when unsorted", () => {
    const out = sortRows(ROWS, null, ACC);
    expect(out).toEqual(ROWS);
    expect(out).not.toBe(ROWS);
  });

  it("sorts numbers stably with nulls last in both directions", () => {
    expect(names(sortRows(ROWS, { key: "n", dir: "asc" }, ACC))).toEqual(["A2", "b", "c", "a10"]);
    expect(names(sortRows(ROWS, { key: "n", dir: "desc" }, ACC))).toEqual(["b", "c", "A2", "a10"]);
  });

  it("keeps input order among multiple nulls", () => {
    const rows = [
      { name: "x", n: null },
      { name: "y", n: 1 },
      { name: "z", n: null },
    ];
    expect(names(sortRows(rows, { key: "n", dir: "asc" }, ACC))).toEqual(["y", "x", "z"]);
  });

  it("sorts strings case-insensitively with numeric collation", () => {
    expect(names(sortRows(ROWS, { key: "name", dir: "asc" }, ACC))).toEqual(["A2", "a10", "b", "c"]);
    expect(names(sortRows(ROWS, { key: "name", dir: "desc" }, ACC))).toEqual(["c", "b", "a10", "A2"]);
  });
});

describe("loadSort / saveSort", () => {
  const KEYS = ["a", "b"] as const;

  it("round-trips and clears", () => {
    expect(loadSort("k", KEYS)).toBeNull();
    saveSort("k", { key: "b", dir: "asc" });
    expect(loadSort("k", KEYS)).toEqual({ key: "b", dir: "asc" });
    saveSort("k", null);
    expect(localStorage.getItem("k")).toBeNull();
  });

  it("rejects malformed or unknown values", () => {
    for (const raw of ["{", "null", "3", '{"key":"zz","dir":"asc"}', '{"key":"a","dir":"up"}']) {
      localStorage.setItem("k", raw);
      expect(loadSort("k", KEYS)).toBeNull();
    }
  });

  it("ignores storage errors", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("quota");
    });
    expect(() => saveSort("k", { key: "a", dir: "asc" })).not.toThrow();
  });
});

function run(o: Partial<FleetRunSummary>): FleetRunSummary {
  const now = Math.floor(Date.now() / 1000);
  return {
    routine_id: "r1",
    routine_title: "Alpha",
    workbench: "wb",
    started_at: now - 100,
    started_at_local: "",
    finished_at: now - 90,
    finished_at_local: "",
    status: "success",
    exit_code: 0,
    ...o,
  };
}

function renderWith(page: React.ReactNode, key: unknown[], runs: FleetRunSummary[]) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  qc.setQueryData(key, runs);
  render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{page}</MemoryRouter>
    </QueryClientProvider>,
  );
}

const linkOrder = (table: HTMLElement) =>
  within(table)
    .getAllByRole("link")
    .map((a) => a.textContent)
    .filter((t) => t !== "VIEW RUN");

describe("sortable fleet tables", () => {
  it("sorts the Runs table by header click, exposes aria-sort, and persists", () => {
    const now = Math.floor(Date.now() / 1000);
    const runs = [
      run({ workbench: "w1", routine_title: "Bravo", started_at: now - 50, finished_at: now - 40 }),
      run({ workbench: "w2", routine_title: "Alpha", started_at: now - 60, finished_at: now - 10 }),
      run({ workbench: "w3", routine_title: "Charlie", started_at: now - 70, finished_at: null, status: "running" }),
    ];
    renderWith(<RunsPage />, ["routines", "runs", 100], runs);
    const table = screen.getByRole("table");
    expect(linkOrder(table)).toEqual(["Bravo", "Alpha", "Charlie"]);

    const durationBtn = screen.getByRole("button", { name: "DURATION" });
    fireEvent.click(durationBtn);
    expect(linkOrder(table)).toEqual(["Alpha", "Bravo", "Charlie"]);
    expect(durationBtn.closest("th")).toHaveAttribute("aria-sort", "descending");
    expect(JSON.parse(localStorage.getItem("moadim.client.runs.sort") ?? "")).toEqual({
      key: "duration",
      dir: "desc",
    });

    fireEvent.click(screen.getByRole("button", { name: "DURATION ▼" }));
    expect(screen.getByRole("button", { name: "DURATION ▲" }).closest("th")).toHaveAttribute("aria-sort", "ascending");
    expect(linkOrder(table)).toEqual(["Bravo", "Alpha", "Charlie"]);

    fireEvent.click(screen.getByRole("button", { name: "ROUTINE" }));
    expect(linkOrder(table)).toEqual(["Alpha", "Bravo", "Charlie"]);
    for (const name of ["STARTED", "STATUS", "EXIT CODE"]) fireEvent.click(screen.getByRole("button", { name }));
    expect(screen.getByRole("button", { name: "ROUTINE" }).closest("th")).toHaveAttribute("aria-sort", "none");
  });

  it("restores a persisted Reliability sort and cycles every column", () => {
    localStorage.setItem("moadim.client.reliability.sort", JSON.stringify({ key: "routine", dir: "asc" }));
    const now = Math.floor(Date.now() / 1000);
    const runs = [
      run({ routine_id: "z", routine_title: "Zulu", workbench: "z1", status: "failed", exit_code: 1 }),
      run({ routine_id: "a", routine_title: "Able", workbench: "a1", started_at: now - 500, finished_at: now - 100 }),
      // Finished but untimed: its p95 is null, so it sinks below timed routines.
      run({ routine_id: "m", routine_title: "Mike", workbench: "m1", started_at: now - 30, finished_at: null }),
    ];
    renderWith(<ReliabilityPage />, ["routines", "runs", 300], runs);
    const table = screen.getAllByRole("table").at(-1) as HTMLElement;
    expect(linkOrder(table)).toEqual(["Able", "Mike", "Zulu"]);
    expect(screen.getByRole("button", { name: "ROUTINE ▲" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "STREAK" }));
    expect(linkOrder(table)).toEqual(["Zulu", "Able", "Mike"]);
    fireEvent.click(screen.getByRole("button", { name: "P95" }));
    expect(linkOrder(table)).toEqual(["Able", "Zulu", "Mike"]);
    for (const name of ["SUCCESS RATE", "P50", "TREND"]) fireEvent.click(screen.getByRole("button", { name }));
    expect(screen.getByRole("button", { name: "TREND ▼" })).toBeInTheDocument();
  });
});

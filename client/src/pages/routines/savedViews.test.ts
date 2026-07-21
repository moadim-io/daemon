// Ported 1:1 from ui/src/routines/saved_views_tests.rs (removed) (the snapshot codec only — the
// localStorage round-trip and SavedViewsBar UI aren't covered there either).
import { describe, expect, it } from "vitest";
import { defaultRoutineFilter, type RoutineFilter } from "./filter";
import { captureSnapshot, decodeSnapshot, type ViewSnapshot } from "./savedViews";

describe("savedViews snapshot codec", () => {
  it("capture/decode round-trips default state", () => {
    const filter = defaultRoutineFilter();
    const snapshot = captureSnapshot(filter, undefined, "asc", "none");
    const { filter: decodedFilter, sortCol, sortDir, groupBy } = decodeSnapshot(snapshot);
    expect(decodedFilter).toEqual(filter);
    expect(sortCol).toBeUndefined();
    expect(sortDir).toBe("asc");
    expect(groupBy).toBe("none");
  });

  it("capture/decode round-trips populated state", () => {
    const filter: RoutineFilter = {
      query: "deploy",
      status: "snoozed",
      agent: { kind: "named", value: "claude" },
      machine: { kind: "machine", value: "box-1" },
      repository: { kind: "named", value: "org/repo" },
      tag: { kind: "named", value: "nightly" },
    };
    const snapshot = captureSnapshot(filter, "health", "desc", "agent");
    const { filter: decodedFilter, sortCol, sortDir, groupBy } = decodeSnapshot(snapshot);
    expect(decodedFilter).toEqual(filter);
    expect(sortCol).toBe("health");
    expect(sortDir).toBe("desc");
    expect(groupBy).toBe("agent");
  });

  it("decode falls back to defaults for unknown tokens", () => {
    const snapshot: ViewSnapshot = {
      query: "",
      status: "not-a-real-status",
      agent: " bogus",
      machine: "some-machine",
      repository: " bogus",
      tag: " bogus",
      sortCol: "not-a-real-col",
      sortDir: "sideways",
      groupBy: "not-a-real-group",
    };
    const { filter, sortCol, sortDir, groupBy } = decodeSnapshot(snapshot);
    expect(filter.status).toBe("all");
    expect(filter.machine).toEqual({ kind: "machine", value: "some-machine" });
    expect(sortCol).toBeUndefined();
    expect(sortDir).toBe("asc");
    expect(groupBy).toBe("none");
  });

  it("decode missing sort_col yields undefined", () => {
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
    const { sortCol } = decodeSnapshot(snapshot);
    expect(sortCol).toBeUndefined();
  });
});

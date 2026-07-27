import { beforeEach, describe, expect, it } from "vitest";
import { loadCollapsedGroups, saveCollapsedGroups } from "./groupCollapse";

describe("loadCollapsedGroups / saveCollapsedGroups", () => {
  beforeEach(() => localStorage.clear());

  it("returns an empty set when nothing is persisted", () => {
    expect(loadCollapsedGroups()).toEqual(new Set());
  });

  it("round-trips a saved set", () => {
    const groups = new Set(["agent:claude", "folder:backend/db"]);
    saveCollapsedGroups(groups);
    expect(loadCollapsedGroups()).toEqual(groups);
  });

  it("ignores corrupt storage", () => {
    localStorage.setItem("moadim.routines.collapsedGroups", "{not json");
    expect(loadCollapsedGroups()).toEqual(new Set());
  });

  it("ignores a persisted value that isn't an array", () => {
    localStorage.setItem("moadim.routines.collapsedGroups", JSON.stringify({ not: "an array" }));
    expect(loadCollapsedGroups()).toEqual(new Set());
  });

  it("drops non-string entries from a persisted array", () => {
    localStorage.setItem("moadim.routines.collapsedGroups", JSON.stringify(["agent:claude", 42, null]));
    expect(loadCollapsedGroups()).toEqual(new Set(["agent:claude"]));
  });
});

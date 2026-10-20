import { describe, expect, it } from "vitest";
import { defaultRoutineFilter, isFilterActive, matchesFilter, parseMachineFacet } from "./filter";
import { captureSnapshot, decodeSnapshot } from "./savedViews";
import { applyViewToParams, paramsToSnapshot } from "./urlView";
import type { RoutineResponse } from "../../api/hooks";

describe("multiple machine filters", () => {
  it("ORs machines and unassigned, ANDs other facets, and treats empty as Any", () => {
    const filter = { ...defaultRoutineFilter(), machine: parseMachineFacet(["a", "b", "\0unassigned"]), status: "enabled" as const };
    const matches = (machines: string[], enabled = true) => matchesFilter(filter, { machines, enabled } as RoutineResponse, new Date(), 0);
    expect(matches(["a"])).toBe(true);
    expect(matches(["b"])).toBe(true);
    expect(matches(["a", "b"])).toBe(true);
    expect(matches([])).toBe(true);
    expect(matches([" "])).toBe(true);
    expect(matches(["c"])).toBe(false);
    expect(matches(["a"], false)).toBe(false);
    expect(isFilterActive({ ...defaultRoutineFilter(), machine: parseMachineFacet([]) })).toBe(false);
  });
  it("normalizes duplicate selections and legacy Any sentinels", () => {
    expect(parseMachineFacet(["\0any", "a", "a"])).toEqual({ kind: "machine", value: "a" });
    expect(parseMachineFacet(["\0any"])).toEqual({ kind: "any" });
    expect(parseMachineFacet(["\0unassigned"])).toEqual({ kind: "unassigned" });
  });
  it("ANDs selected machines with query, agent, repository and tag", () => {
    const routine = { machines: ["b"], title: "Deploy", schedule: "@daily", agent: "codex",
      repositories: [{ repository: "org/repo" }], tags: ["ops"] } as RoutineResponse;
    const filter = { ...defaultRoutineFilter(), machine: parseMachineFacet(["a", "b"]) };
    for (const [key, value] of [["agent", "codex"], ["repository", "org/repo"], ["tag", "ops"]] as const) {
      expect(matchesFilter({ ...filter, [key]: { kind: "named", value } }, routine, new Date(), 0)).toBe(true);
      expect(matchesFilter({ ...filter, [key]: { kind: "named", value: "missing" } }, routine, new Date(), 0)).toBe(false);
    }
    expect(matchesFilter({ ...filter, query: "deploy" }, routine, new Date(), 0)).toBe(true);
    expect(matchesFilter({ ...filter, query: "missing" }, routine, new Date(), 0)).toBe(false);
  });
  it.each(["any", "None", "a,b", "a&b+雪", '["a"]', "\0unassigned", "\0any"])("preserves legacy scalar %s", (machine) => {
    const snapshot = { ...captureSnapshot(defaultRoutineFilter(), undefined, "asc", "none"), machine };
    expect(decodeSnapshot(paramsToSnapshot(new URLSearchParams({ machine }))!).filter.machine).toEqual(parseMachineFacet(machine));
    expect(decodeSnapshot(JSON.parse(JSON.stringify(snapshot))).filter.machine).toEqual(parseMachineFacet(machine));
  });
  it("round trips arrays through storage and merged URLs without losing unrelated params", () => {
    const machine = parseMachineFacet(["a,b", "a&b+雪", "None", "\0unassigned"]);
    const snapshot = captureSnapshot({ ...defaultRoutineFilter(), machine }, undefined, "asc", "none");
    const params = applyViewToParams(new URLSearchParams("extra=keep&machine=old"), snapshot);
    expect(params.get("extra")).toBe("keep");
    expect(params.getAll("machine")).toHaveLength(4);
    expect(decodeSnapshot(paramsToSnapshot(params)!).filter.machine).toEqual(machine);
    expect(decodeSnapshot(JSON.parse(JSON.stringify(snapshot))).filter.machine).toEqual(machine);
  });
});

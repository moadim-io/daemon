import { describe, expect, it } from "vitest";
import { defaultRoutineFilter } from "./filter";
import { captureSnapshot, decodeSnapshot } from "./savedViews";
import { applyViewToParams, paramsToSnapshot, snapshotToParams } from "./urlView";

describe("snapshotToParams", () => {
  it("produces no params for the default (unfiltered) snapshot", () => {
    const snapshot = captureSnapshot(defaultRoutineFilter(), undefined, "asc", "none");
    expect([...snapshotToParams(snapshot)]).toEqual([]);
  });

  it("only includes fields that differ from their default", () => {
    const snapshot = captureSnapshot({ ...defaultRoutineFilter(), query: "flaky" }, "title", "desc", "agent");
    const params = snapshotToParams(snapshot);
    expect(params.get("q")).toBe("flaky");
    expect(params.get("sort")).toBe("title");
    expect(params.get("dir")).toBe("desc");
    expect(params.get("group")).toBe("agent");
    expect(params.has("status")).toBe(false);
    expect(params.has("agent")).toBe(false);
  });
});

describe("paramsToSnapshot", () => {
  it("returns undefined when no recognized view params are present", () => {
    expect(paramsToSnapshot(new URLSearchParams("history=abc"))).toBeUndefined();
    expect(paramsToSnapshot(new URLSearchParams())).toBeUndefined();
  });

  it("decodes back into a snapshot that round-trips through decodeSnapshot", () => {
    const params = new URLSearchParams("q=flaky&sort=title&dir=desc&group=agent");
    const snapshot = paramsToSnapshot(params);
    expect(snapshot).toBeDefined();
    const decoded = decodeSnapshot(snapshot!);
    expect(decoded.filter.query).toBe("flaky");
    expect(decoded.sortCol).toBe("title");
    expect(decoded.sortDir).toBe("desc");
    expect(decoded.groupBy).toBe("agent");
  });

  it("full snapshot -> params -> snapshot round-trips exactly for a non-default view", () => {
    const original = captureSnapshot(
      { ...defaultRoutineFilter(), query: "prod", status: "enabled" },
      "health",
      "desc",
      "machine",
    );
    const roundTripped = paramsToSnapshot(snapshotToParams(original));
    expect(roundTripped).toEqual(original);
  });
});

describe("applyViewToParams", () => {
  it("adds view params while preserving unrelated existing params", () => {
    const prev = new URLSearchParams("history=abc");
    const snapshot = captureSnapshot({ ...defaultRoutineFilter(), query: "x" }, undefined, "asc", "none");
    const next = applyViewToParams(prev, snapshot);
    expect(next.get("history")).toBe("abc");
    expect(next.get("q")).toBe("x");
  });

  it("clears stale view params when the new snapshot is back to default", () => {
    const prev = new URLSearchParams("q=old&sort=title&dir=desc&group=agent");
    const next = applyViewToParams(prev, captureSnapshot(defaultRoutineFilter(), undefined, "asc", "none"));
    expect([...next.keys()]).toEqual([]);
  });
});

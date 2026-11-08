import { describe, expect, it } from "vitest";
import { severityCounts, severityOf, stepProblem } from "./logSeverity";

describe("severityOf", () => {
  it.each([
    ["ERROR: connection refused", "error"],
    ["thread 'main' panicked at src/x.rs", "error"],
    ["Traceback (most recent call last):", "error"],
    ["test foo ... FAILED", "error"],
    ["fatal: not a git repository", "error"],
    ["WARN retrying in 5s", "warn"],
    ["warning: unused variable", "warn"],
    ["this API is deprecated", "warn"],
    ["warning: build failed", "error"],
  ])("%s → %s", (line, sev) => {
    expect(severityOf(line)).toBe(sev);
  });

  it("ignores substrings inside other words and plain lines", () => {
    expect(severityOf("terrorform apply")).toBeUndefined();
    expect(severityOf("forewarned is forearmed")).toBeUndefined();
    expect(severityOf("INFO: server started")).toBeUndefined();
  });
});

describe("severityCounts", () => {
  it("counts each line once by its top severity", () => {
    expect(severityCounts(["error a", "warn b", "warn c", "ok", "warning + error"])).toEqual({ error: 2, warn: 2 });
  });
});

describe("stepProblem", () => {
  const p = [3, 7, 12];
  it("returns undefined with no problems", () => {
    expect(stepProblem([], 1, 1)).toBeUndefined();
  });
  it("starts at the first / last when nothing is selected", () => {
    expect(stepProblem(p, undefined, 1)).toBe(3);
    expect(stepProblem(p, undefined, -1)).toBe(12);
  });
  it("moves forward and backward", () => {
    expect(stepProblem(p, 3, 1)).toBe(7);
    expect(stepProblem(p, 5, 1)).toBe(7);
    expect(stepProblem(p, 7, -1)).toBe(3);
    expect(stepProblem(p, 10, -1)).toBe(7);
  });
  it("wraps around both ends", () => {
    expect(stepProblem(p, 12, 1)).toBe(3);
    expect(stepProblem(p, 3, -1)).toBe(12);
  });
});

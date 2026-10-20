import { describe, expect, it } from "vitest";
import { diffCounts, diffLogs } from "./logDiff";

describe("diffLogs", () => {
  it("returns all-equal ops for identical content", () => {
    const ops = diffLogs("a\nb\nc\n", "a\nb\nc\n");
    expect(ops).toEqual([
      { kind: "equal", line: "a" },
      { kind: "equal", line: "b" },
      { kind: "equal", line: "c" },
    ]);
  });

  it("detects a single changed line as remove+add around shared context", () => {
    const ops = diffLogs("a\nb\nc\n", "a\nx\nc\n");
    expect(ops).toEqual([
      { kind: "equal", line: "a" },
      { kind: "remove", line: "b" },
      { kind: "add", line: "x" },
      { kind: "equal", line: "c" },
    ]);
  });

  it("detects an appended line", () => {
    const ops = diffLogs("a\nb\n", "a\nb\nc\n");
    expect(ops).toEqual([
      { kind: "equal", line: "a" },
      { kind: "equal", line: "b" },
      { kind: "add", line: "c" },
    ]);
  });

  it("handles empty inputs", () => {
    expect(diffLogs("", "")).toEqual([]);
    expect(diffLogs("", "a\n")).toEqual([{ kind: "add", line: "a" }]);
    expect(diffLogs("a\n", "")).toEqual([{ kind: "remove", line: "a" }]);
  });

  it("falls back to a flat diff above the LCS line cap", () => {
    const big = Array.from({ length: 4001 }, (_, i) => `line${i}`).join("\n") + "\n";
    const other = "different\n";
    const ops = diffLogs(big, other);
    expect(ops.every((op) => op.kind !== "equal")).toBe(true);
    expect(ops.filter((op) => op.kind === "remove")).toHaveLength(4001);
    expect(ops.filter((op) => op.kind === "add")).toHaveLength(1);
  });
});

describe("diffCounts", () => {
  it("counts added and removed lines, ignoring equal ones", () => {
    const ops = diffLogs("a\nb\nc\n", "a\nx\nc\nd\n");
    expect(diffCounts(ops)).toEqual({ added: 2, removed: 1 });
  });

  it("returns zeroes for no ops", () => {
    expect(diffCounts([])).toEqual({ added: 0, removed: 0 });
  });
});

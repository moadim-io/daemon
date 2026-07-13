import { describe, expect, it } from "vitest";
import { fmtRetention, fmtRunDuration, runStatusClass, runStatusLabel } from "./runDisplay";

describe("runStatusClass", () => {
  it("covers every variant", () => {
    expect(runStatusClass("running")).toBe("run-status running");
    expect(runStatusClass("success")).toBe("run-status success");
    expect(runStatusClass("failed")).toBe("run-status failed");
    expect(runStatusClass("unknown")).toBe("run-status unknown");
  });
});

describe("runStatusLabel", () => {
  it("covers every variant", () => {
    expect(runStatusLabel("running")).toBe("RUNNING");
    expect(runStatusLabel("success")).toBe("SUCCESS");
    expect(runStatusLabel("failed")).toBe("FAILED");
    expect(runStatusLabel("unknown")).toBe("UNKNOWN");
  });
});

describe("fmtRunDuration", () => {
  it("is seconds under a minute", () => {
    expect(fmtRunDuration(1_000, 1_045)).toBe("45s");
  });

  it("is minutes at the exact minute boundary", () => {
    expect(fmtRunDuration(0, 60)).toBe("1m");
  });

  it("is minutes under an hour", () => {
    expect(fmtRunDuration(0, 754)).toBe("12m");
  });

  it("is hours and minutes at the exact hour boundary", () => {
    expect(fmtRunDuration(0, 3_600)).toBe("1h 0m");
  });

  it("is hours and minutes over an hour", () => {
    expect(fmtRunDuration(0, 7_530)).toBe("2h 5m");
  });

  it("saturates at 0s when finished precedes started", () => {
    expect(fmtRunDuration(100, 50)).toBe("0s");
  });
});

describe("fmtRetention", () => {
  it("reads under a minute", () => {
    expect(fmtRetention(1_000, 1_030)).toBe("expires in <1m");
  });

  it("is minutes under an hour", () => {
    expect(fmtRetention(0, 754)).toBe("expires in 12m");
  });

  it("is hours and minutes over an hour", () => {
    expect(fmtRetention(0, 7_530)).toBe("expires in 2h 5m");
  });

  it("reads expired once the deadline has passed", () => {
    expect(fmtRetention(100, 50)).toBe("expired");
    expect(fmtRetention(100, 100)).toBe("expired");
  });
});

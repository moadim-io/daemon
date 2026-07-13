import { beforeEach, describe, expect, it } from "vitest";
import { fmtFreshness, loadRefreshToken, refreshMs, saveRefreshToken } from "./RefreshControl";

describe("refreshMs", () => {
  it("off has no cadence, others do", () => {
    expect(refreshMs("off")).toBeUndefined();
    expect(refreshMs("5")).toBe(5_000);
    expect(refreshMs("15")).toBe(15_000);
    expect(refreshMs("30")).toBe(30_000);
    expect(refreshMs("60")).toBe(60_000);
  });
});

describe("fmtFreshness", () => {
  it("is 'just now' under a minute", () => {
    expect(fmtFreshness(0)).toBe("updated just now");
    expect(fmtFreshness(59)).toBe("updated just now");
  });

  it("is minutes under an hour", () => {
    expect(fmtFreshness(60)).toBe("updated 1m ago");
    expect(fmtFreshness(3_599)).toBe("updated 59m ago");
  });

  it("is hours at/beyond an hour", () => {
    expect(fmtFreshness(3_600)).toBe("updated 1h ago");
    expect(fmtFreshness(7_200)).toBe("updated 2h ago");
  });
});

describe("loadRefreshToken / saveRefreshToken", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("defaults to off when nothing is persisted", () => {
    expect(loadRefreshToken()).toBe("off");
  });

  it("defaults to off for garbage values", () => {
    localStorage.setItem("moadim.refresh-interval", "nonsense");
    expect(loadRefreshToken()).toBe("off");
  });

  it("round-trips every token", () => {
    for (const token of ["off", "5", "15", "30", "60"] as const) {
      saveRefreshToken(token);
      expect(loadRefreshToken()).toBe(token);
    }
  });
});

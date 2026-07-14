import { describe, expect, it } from "vitest";
import { formatTtl } from "./ttl";

describe("formatTtl", () => {
  it("renders null/undefined as the server default", () => {
    expect(formatTtl(null)).toBe("default");
    expect(formatTtl(undefined)).toBe("default");
  });

  it("renders zero as 0s rather than dividing into a unit", () => {
    expect(formatTtl(0)).toBe("0s");
  });

  it("picks the largest whole unit that divides the value evenly", () => {
    expect(formatTtl(172_800)).toBe("2d");
    expect(formatTtl(7_200)).toBe("2h");
    expect(formatTtl(120)).toBe("2m");
  });

  it("falls back to raw seconds when no larger unit divides evenly", () => {
    expect(formatTtl(90)).toBe("90s");
    expect(formatTtl(45)).toBe("45s");
  });
});

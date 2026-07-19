import { describe, expect, it } from "vitest";
import { humanizeBytes } from "./bytes";

describe("humanizeBytes", () => {
  it("renders sub-KB as a bare integer", () => {
    expect(humanizeBytes(0)).toBe("0 B");
    expect(humanizeBytes(1023)).toBe("1023 B");
  });

  it("renders larger sizes with one decimal", () => {
    expect(humanizeBytes(1024)).toBe("1.0 KB");
    expect(humanizeBytes(12 * 1024 * 1024 + 400 * 1024)).toBe("12.4 MB");
  });

  it("caps at TB", () => {
    expect(humanizeBytes(1024 ** 5)).toBe("1024.0 TB");
  });
});

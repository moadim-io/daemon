import { describe, expect, it } from "vitest";
import { describeCronLive, normalizeCron, reltime } from "./cronUtils";

describe("normalizeCron", () => {
  it("passes through empty/blank", () => {
    expect(normalizeCron("")).toBe("");
    expect(normalizeCron("   ")).toBe("");
  });

  it("passes through 5-field", () => {
    expect(normalizeCron("0 * * * *")).toBe("0 * * * *");
  });

  it("passes through 6-field (seconds) unchanged", () => {
    expect(normalizeCron("30 0 * * * *")).toBe("30 0 * * * *");
  });

  it("drops seconds and year from a 7-field expression", () => {
    expect(normalizeCron("30 0 12 * * * 2026")).toBe("0 12 * * *");
  });

  it("passes through @-keywords", () => {
    expect(normalizeCron("@daily")).toBe("@daily");
  });
});

describe("describeCronLive", () => {
  it("reports a placeholder for blank input", () => {
    expect(describeCronLive("   ")).toEqual([false, "— enter a cron expression —"]);
  });

  it("reports invalid for a bad expression", () => {
    const [valid, description] = describeCronLive("not a cron");
    expect(valid).toBe(false);
    expect(description).toBe("Invalid cron expression");
  });

  it("describes a valid expression", () => {
    const [valid, description] = describeCronLive("0 * * * *");
    expect(valid).toBe(true);
    expect(description.length).toBeGreaterThan(0);
  });

  it("describes a normalized 7-field expression", () => {
    const [valid, description] = describeCronLive("30 0 12 * * * 2026");
    expect(valid).toBe(true);
    expect(description).toContain("12:00");
  });
});

describe("reltime", () => {
  it("returns a dash for 0", () => {
    expect(reltime(0)).toBe("—");
  });

  it("returns 'just now' for the current time", () => {
    expect(reltime(Math.floor(Date.now() / 1000))).toBe("just now");
  });

  it("formats minutes/hours/days ago", () => {
    const now = Math.floor(Date.now() / 1000);
    expect(reltime(now - 5 * 60)).toBe("5m ago");
    expect(reltime(now - 3 * 3_600)).toBe("3h ago");
    expect(reltime(now - 2 * 86_400)).toBe("2d ago");
  });
});

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyTheme, loadThemeLight, saveThemeLight } from "./theme";

beforeEach(() => {
  localStorage.clear();
  document.documentElement.classList.remove("theme-light");
});

describe("theme", () => {
  it("defaults to dark when nothing is stored", () => {
    expect(loadThemeLight()).toBe(false);
  });

  it("round-trips a saved preference", () => {
    saveThemeLight(true);
    expect(loadThemeLight()).toBe(true);
    saveThemeLight(false);
    expect(loadThemeLight()).toBe(false);
  });

  it("toggles the theme-light class on <html>", () => {
    applyTheme(true);
    expect(document.documentElement.classList.contains("theme-light")).toBe(true);
    applyTheme(false);
    expect(document.documentElement.classList.contains("theme-light")).toBe(false);
  });

  describe("when localStorage throws (private mode / quota)", () => {
    afterEach(() => {
      vi.restoreAllMocks();
    });

    it("falls back to dark instead of propagating the error", () => {
      vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
        throw new DOMException("blocked", "SecurityError");
      });
      expect(loadThemeLight()).toBe(false);
    });

    it("saveThemeLight swallows the error instead of propagating it", () => {
      vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
        throw new DOMException("blocked", "SecurityError");
      });
      expect(() => saveThemeLight(true)).not.toThrow();
    });
  });
});

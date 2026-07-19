import { beforeEach, describe, expect, it } from "vitest";
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
});

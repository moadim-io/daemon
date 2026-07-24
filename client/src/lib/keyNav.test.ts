import { describe, expect, it } from "vitest";
import { isEditableTarget, NAV_CHORDS, routeForChordKey } from "./keyNav";

describe("routeForChordKey", () => {
  it("resolves every bound chord letter to its route", () => {
    expect(routeForChordKey("o")).toBe("/");
    expect(routeForChordKey("r")).toBe("/routines");
    expect(routeForChordKey("h")).toBe("/heatmap");
    expect(routeForChordKey("l")).toBe("/reliability");
    expect(routeForChordKey("m")).toBe("/machines");
    expect(routeForChordKey("s")).toBe("/settings");
  });

  it("is case-insensitive", () => {
    expect(routeForChordKey("R")).toBe("/routines");
  });

  it("returns undefined for an unbound key", () => {
    expect(routeForChordKey("z")).toBeUndefined();
  });

  it("covers every declared chord exactly once", () => {
    const keys = NAV_CHORDS.map((c) => c.key);
    expect(new Set(keys).size).toBe(keys.length);
  });
});

describe("isEditableTarget", () => {
  it("treats input, textarea, and select as editable", () => {
    expect(isEditableTarget(document.createElement("input"))).toBe(true);
    expect(isEditableTarget(document.createElement("textarea"))).toBe(true);
    expect(isEditableTarget(document.createElement("select"))).toBe(true);
  });

  it("treats a contenteditable element as editable", () => {
    const div = document.createElement("div");
    div.setAttribute("contenteditable", "true");
    expect(isEditableTarget(div)).toBe(true);
  });

  it("treats a plain element as not editable", () => {
    expect(isEditableTarget(document.createElement("div"))).toBe(false);
  });

  it("treats null and non-element targets as not editable", () => {
    expect(isEditableTarget(null)).toBe(false);
    expect(isEditableTarget({} as EventTarget)).toBe(false);
  });
});

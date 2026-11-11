import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PINS_KEY, loadPins, savePins, togglePin, usePins } from "./pins";

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("pins storage", () => {
  it("defaults to no pins", () => {
    expect(loadPins()).toEqual([]);
  });

  it("toggles pins on in order and off again", () => {
    togglePin("a");
    togglePin("b");
    expect(loadPins()).toEqual(["a", "b"]);
    togglePin("a");
    expect(loadPins()).toEqual(["b"]);
  });

  it("returns a stable array while storage is unchanged", () => {
    savePins(["x"]);
    expect(loadPins()).toBe(loadPins());
  });

  it("ignores corrupt or foreign storage values", () => {
    localStorage.setItem(PINS_KEY, "{not json");
    expect(loadPins()).toEqual([]);
    localStorage.setItem(PINS_KEY, JSON.stringify({ a: 1 }));
    expect(loadPins()).toEqual([]);
    localStorage.setItem(PINS_KEY, JSON.stringify(["ok", 3, null]));
    expect(loadPins()).toEqual(["ok"]);
  });

  it("survives unavailable storage", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied");
    });
    expect(loadPins()).toEqual([]);
    expect(() => savePins(["a"])).not.toThrow();
  });
});

describe("usePins", () => {
  it("re-renders on same-tab toggles", () => {
    const { result } = renderHook(() => usePins());
    expect(result.current.isPinned("a")).toBe(false);
    act(() => result.current.toggle("a"));
    expect(result.current.pins).toEqual(["a"]);
    expect(result.current.isPinned("a")).toBe(true);
  });

  it("picks up changes made in another tab and ignores unrelated keys", () => {
    const { result } = renderHook(() => usePins());
    act(() => {
      localStorage.setItem(PINS_KEY, JSON.stringify(["remote"]));
      window.dispatchEvent(new StorageEvent("storage", { key: "other" }));
    });
    act(() => {
      window.dispatchEvent(new StorageEvent("storage", { key: PINS_KEY }));
    });
    expect(result.current.pins).toEqual(["remote"]);
    act(() => {
      localStorage.clear();
      window.dispatchEvent(new StorageEvent("storage", { key: null }));
    });
    expect(result.current.pins).toEqual([]);
  });
});

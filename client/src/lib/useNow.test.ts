import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useNow } from "./useNow";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useNow", () => {
  it("returns the current time immediately", () => {
    vi.setSystemTime(1_000_000);
    const { result } = renderHook(() => useNow());
    expect(result.current).toBe(1_000_000);
  });

  it("ticks on the given interval", () => {
    vi.setSystemTime(0);
    const { result } = renderHook(() => useNow(1_000));
    act(() => vi.advanceTimersByTime(1_000));
    expect(result.current).toBe(1_000);
  });
});

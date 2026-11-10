/**
 * Personal "pinned routines" watchlist (GitHub Actions pinned workflows / CloudWatch favorites
 * pattern). Pins live in this browser's localStorage only and sync across open tabs.
 */
import { useCallback, useSyncExternalStore } from "react";

export const PINS_KEY = "moadim.client.pinned_routines";
const PINS_CHANGE_EVENT = "moadim-pins-change";

const EMPTY: readonly string[] = [];
let cachedRaw: string | null = null;
let cachedPins: readonly string[] = EMPTY;

function readRaw(): string | null {
  try {
    return localStorage.getItem(PINS_KEY);
  } catch {
    return null;
  }
}

function parse(raw: string | null): readonly string[] {
  if (raw === null) return EMPTY;
  try {
    const v: unknown = JSON.parse(raw);
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : EMPTY;
  } catch {
    return EMPTY;
  }
}

/** Pinned routine ids, in pin order. Returns a stable array while storage is unchanged. */
export function loadPins(): readonly string[] {
  const raw = readRaw();
  if (raw !== cachedRaw) {
    cachedRaw = raw;
    cachedPins = parse(raw);
  }
  return cachedPins;
}

/** Persists the pin list (best-effort) and notifies same-tab subscribers. */
export function savePins(ids: readonly string[]): void {
  try {
    localStorage.setItem(PINS_KEY, JSON.stringify(ids));
  } catch {
    // storage unavailable (private mode / quota) — nothing to persist to
  }
  window.dispatchEvent(new Event(PINS_CHANGE_EVENT));
}

/** Adds `id` to the end of the pin list, or removes it when already pinned. */
export function togglePin(id: string): void {
  const pins = loadPins();
  savePins(pins.includes(id) ? pins.filter((p) => p !== id) : [...pins, id]);
}

function subscribe(onChange: () => void): () => void {
  const onStorage = (e: StorageEvent) => {
    if (e.key === PINS_KEY || e.key === null) onChange();
  };
  window.addEventListener(PINS_CHANGE_EVENT, onChange);
  window.addEventListener("storage", onStorage);
  return () => {
    window.removeEventListener(PINS_CHANGE_EVENT, onChange);
    window.removeEventListener("storage", onStorage);
  };
}

/** Live pin list plus a toggle; re-renders on changes from this tab or another one. */
export function usePins(): {
  pins: readonly string[];
  isPinned: (id: string) => boolean;
  toggle: (id: string) => void;
} {
  const pins = useSyncExternalStore(subscribe, loadPins);
  const isPinned = useCallback((id: string) => pins.includes(id), [pins]);
  return { pins, isPinned, toggle: togglePin };
}

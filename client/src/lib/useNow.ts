import { useEffect, useState } from "react";

/**
 * Current wall-clock time (ms since epoch), ticking every `intervalMs` via a
 * timer effect. Centralizes the "read `Date.now()` outside of render" shape
 * `react-hooks/purity` requires: render must stay a pure function of props/
 * state, so the impure clock read lives in an effect's timer callback
 * instead of in the render body (previously duplicated ad hoc by
 * `RefreshControl` and inlined, non-ticking, in a couple of freshness
 * labels).
 */
export function useNow(intervalMs = 1_000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs]);
  return now;
}

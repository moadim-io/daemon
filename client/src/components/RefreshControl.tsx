import { useNow } from "../lib/useNow";

/**
 * Live auto-refresh control for a data table's action row: a Grafana/Datadog
 * -style interval dropdown plus an "updated Ns ago" freshness cue. Direct
 * counterpart to `ui/src/refresh.rs (removed)`'s `RefreshControl`, simplified to lean
 * on TanStack Query's `dataUpdatedAt` for freshness instead of tracking a
 * separate timestamp — the interval codec and persistence are still pure/
 * host-tested (see `RefreshControl.test.ts`) to preserve the operator-facing
 * behavior: choose a cadence, see how fresh the data is, both surviving reload.
 */
export type RefreshToken = "off" | "5" | "15" | "30" | "60";

const OPTIONS: { token: RefreshToken; label: string; ms: number | undefined }[] = [
  { token: "off", label: "Off", ms: undefined },
  { token: "5", label: "5s", ms: 5_000 },
  { token: "15", label: "15s", ms: 15_000 },
  { token: "30", label: "30s", ms: 30_000 },
  { token: "60", label: "60s", ms: 60_000 },
];

const STORAGE_KEY = "moadim.refresh-interval";

function isRefreshToken(v: string | null): v is RefreshToken {
  return v === "off" || v === "5" || v === "15" || v === "30" || v === "60";
}

/** The cadence in milliseconds, or `undefined` for "off" (no auto-refresh). */
export function refreshMs(token: RefreshToken): number | undefined {
  return OPTIONS.find((o) => o.token === token)?.ms;
}

/** Read the persisted interval from `localStorage`, defaulting to "off" when unavailable/unrecognized. */
export function loadRefreshToken(): RefreshToken {
  try {
    const token = localStorage.getItem(STORAGE_KEY);
    return isRefreshToken(token) ? token : "off";
  } catch {
    return "off";
  }
}

/** Persist the chosen interval. Best-effort: a storage error is silently ignored. */
export function saveRefreshToken(token: RefreshToken): void {
  try {
    localStorage.setItem(STORAGE_KEY, token);
  } catch {
    // ponytail: private-mode/quota errors are non-fatal — the in-memory choice still applies this session.
  }
}

/** Format "time since last load": "updated just now" under a minute, then "Nm ago" / "Nh ago". */
export function fmtFreshness(secsAgo: number): string {
  if (secsAgo < 60) return "updated just now";
  if (secsAgo < 3_600) return `updated ${Math.floor(secsAgo / 60)}m ago`;
  return `updated ${Math.floor(secsAgo / 3_600)}h ago`;
}

export interface RefreshControlProps {
  /** Currently selected interval. */
  token: RefreshToken;
  /** `dataUpdatedAt` (ms) of the freshest underlying query; `0` hides the freshness cue (not loaded yet). */
  updatedAtMs: number;
  /** Called with the newly chosen token when the operator changes it. */
  onChange: (token: RefreshToken) => void;
}

/** Interval dropdown + live freshness label for a data table's action row. */
export function RefreshControl({ token, updatedAtMs, onChange }: RefreshControlProps) {
  // A local 1s clock re-renders just this widget so "updated Ns ago" stays live
  // without forcing the parent table to re-render every second.
  const now = useNow();

  const freshness = updatedAtMs > 0 ? fmtFreshness(Math.max(0, Math.floor((now - updatedAtMs) / 1000))) : undefined;

  return (
    <div className="refresh-control">
      <label className="refresh-lbl" htmlFor="refresh-interval">
        AUTO
      </label>
      <select
        id="refresh-interval"
        className="refresh-select"
        value={token}
        aria-label="Auto-refresh interval"
        onChange={(e) => onChange(e.target.value as RefreshToken)}
      >
        {OPTIONS.map((o) => (
          <option key={o.token} value={o.token}>
            {o.label}
          </option>
        ))}
      </select>
      {freshness !== undefined && (
        <span className="refresh-fresh" title="Time since the list last refreshed">
          {freshness}
        </span>
      )}
    </div>
  );
}

import type { LockStatus } from "../api/hooks";

export interface GlobalLockBannerProps {
  /** Current lock status; `undefined` hides the banner (status not yet fetched). */
  status: LockStatus | undefined;
  onUnlock: () => void;
}

/** Banner shown above the routine list (and Overview) when the global lock is active. */
export function GlobalLockBanner({ status, onUnlock }: GlobalLockBannerProps) {
  if (!status || !status.locked) return null;
  return (
    <div className="card" style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "10px 14px", marginBottom: 16, borderColor: "var(--warn)" }}>
      <div style={{ fontSize: 13, fontWeight: 600 }}>
        ⚠ Routines globally locked — scheduling and manual triggers paused
        {status.shared && <span style={{ marginLeft: 8, fontSize: 11, color: "var(--text-faint)" }}>SHARED .lock</span>}
        {status.local && <span style={{ marginLeft: 8, fontSize: 11, color: "var(--text-faint)" }}>LOCAL .local.lock</span>}
      </div>
      <button className="btn btn-ghost" onClick={onUnlock}>
        Unlock all
      </button>
    </div>
  );
}

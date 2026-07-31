import type { HealthResponse } from "../../api/hooks";

interface CrontabSyncStatusProps {
  health: HealthResponse | undefined;
  loading: boolean;
  retrying: boolean;
  onRefresh: () => void;
  onRetry: () => void;
}

export function CrontabSyncStatus({
  health,
  loading,
  retrying,
  onRefresh,
  onRetry,
}: CrontabSyncStatusProps) {
  const sync = health?.crontab_sync;
  if (loading && !sync) {
    return <p className="settings-card-copy">Checking OS crontab sync…</p>;
  }

  if (!sync) {
    return (
      <div className="settings-health settings-health-unknown">
        <div className="settings-health-main">Health details unavailable</div>
        <p className="settings-card-copy">Refresh status once the daemon is reachable.</p>
        <button type="button" className="btn btn-secondary" onClick={onRefresh}>
          Refresh status
        </button>
      </div>
    );
  }

  if (sync.ok) {
    return (
      <div className="settings-health settings-health-ok">
        <div className="settings-health-main">OS crontab sync is healthy</div>
        <p className="settings-card-copy">Scheduled routine changes are installed in the OS crontab.</p>
        <button type="button" className="btn btn-secondary" onClick={onRefresh}>
          Refresh status
        </button>
      </div>
    );
  }

  return (
    <div className="settings-health settings-health-warn">
      <div className="settings-health-main">⚠ OS crontab sync needs attention</div>
      <p className="settings-card-copy">
        Scheduled routine changes may not fire until macOS allows Moadim to write the crontab again.
      </p>
      {sync.last_error && <div className="settings-health-error">{sync.last_error}</div>}
      {sync.last_error_at && (
        <div className="settings-health-meta">Last failed: {formatUnixTime(sync.last_error_at)}</div>
      )}
      <ol className="settings-recovery-list" aria-label="Crontab sync recovery steps">
        <li>Open macOS System Settings → Privacy & Security → Full Disk Access.</li>
        <li>Allow the Moadim daemon or the launcher that starts it.</li>
        <li>Restart Moadim if you changed permissions, then click Retry sync now.</li>
      </ol>
      <div className="settings-health-actions">
        <button type="button" className="btn btn-primary" disabled={retrying} onClick={onRetry}>
          {retrying ? "Retrying…" : "Retry sync now"}
        </button>
        <button type="button" className="btn btn-secondary" onClick={onRefresh}>
          Refresh status
        </button>
      </div>
    </div>
  );
}

function formatUnixTime(unixSecs: number): string {
  return new Date(unixSecs * 1000).toLocaleString();
}

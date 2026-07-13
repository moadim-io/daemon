import { useEffect, useState } from "react";
import { useRoutineLogs } from "../../api/hooks";
import { loadRefreshToken, RefreshControl, refreshMs, saveRefreshToken, type RefreshToken } from "../../components/RefreshControl";
import { LogViewer } from "./LogViewer";

export interface RoutineLogsProps {
  id: string;
  title: string;
  onBack: () => void;
}

/** Tail of the routine's most recent run's log, with optional auto-refresh (#357: the daemon
 * periodically reaps finished workbenches, so a stale tail should be able to clear itself). */
export function RoutineLogs({ id, title, onBack }: RoutineLogsProps) {
  const [interval, setInterval_] = useState<RefreshToken>(loadRefreshToken);
  const logs = useRoutineLogs(id);

  useEffect(() => {
    const ms = refreshMs(interval);
    if (ms === undefined) return;
    const timer = window.setInterval(() => void logs.refetch(), ms);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [interval, id]);

  const onSetInterval = (next: RefreshToken) => {
    setInterval_(next);
    saveRefreshToken(next);
  };

  return (
    <main className="logs-page">
      <div className="page-hd">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onBack}>
          ← BACK
        </button>
        <div className="page-title">LOGS / {title}</div>
        <RefreshControl token={interval} updatedAtMs={logs.dataUpdatedAt} onChange={onSetInterval} />
        <button
          type="button"
          className="btn-refresh"
          title="Refresh"
          aria-label="Refresh"
          onClick={() => void logs.refetch()}
        >
          ↻
        </button>
      </div>
      <LogViewer
        content={logs.data}
        loading={logs.isLoading}
        err={logs.isError ? logs.error.message : undefined}
      />
    </main>
  );
}

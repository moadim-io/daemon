import { useState } from "react";
import { useRoutineRuns, useRunLog } from "../../api/hooks";
import { fmtFreshness } from "../../components/RefreshControl";
import { abstime, reltime } from "../../lib/cronUtils";
import { fmtRetention, fmtRunDuration, runStatusClass, runStatusLabel } from "../../lib/runDisplay";
import { useNow } from "../../lib/useNow";
import { LogViewer } from "./LogViewer";

export interface RoutineHistoryProps {
  id: string;
  title: string;
  onBack: () => void;
}

/** Run-history table for one routine, with an expandable inline log viewer per run. */
export function RoutineHistory({ id, title, onBack }: RoutineHistoryProps) {
  const runsQuery = useRoutineRuns(id);
  const [selected, setSelected] = useState<string | undefined>(undefined);
  const logQuery = useRunLog(id, selected ?? "", selected !== undefined);

  const runs = runsQuery.data ?? [];
  const now = useNow();
  const nowSecs = Math.floor(now / 1000);

  return (
    <main className="logs-page">
      <div className="page-hd">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onBack}>
          ← BACK
        </button>
        <div className="page-title">HISTORY / {title}</div>
        {runsQuery.dataUpdatedAt > 0 && (
          <span className="page-freshness">
            {fmtFreshness(Math.max(0, (now - runsQuery.dataUpdatedAt) / 1000))}
          </span>
        )}
        <button
          type="button"
          className="btn-refresh"
          title="Refresh"
          aria-label="Refresh"
          onClick={() => void runsQuery.refetch()}
        >
          ↻
        </button>
      </div>

      {runsQuery.isLoading ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="spinner" />
          </div>
        </div>
      ) : runsQuery.isError ? (
        <div className="logs-error">Error: {runsQuery.error.message}</div>
      ) : runs.length === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⧗</div>
            <div className="empty-msg">NO RUNS YET</div>
          </div>
        </div>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>STARTED</th>
                <th>STATUS</th>
                <th>DURATION</th>
                <th>EXIT CODE</th>
                <th>RETENTION</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {runs.map((run) => {
                const isSelected = selected === run.workbench;
                return (
                  <tr key={run.workbench} className={isSelected ? "row-selected" : ""}>
                    <td>
                      <div className="cell-time" title={`${run.workbench} · ${abstime(run.started_at)}`}>
                        {reltime(run.started_at)}
                      </div>
                    </td>
                    <td>
                      <span className={runStatusClass(run.status)}>{runStatusLabel(run.status)}</span>
                    </td>
                    <td>{run.finished_at != null ? fmtRunDuration(run.started_at, run.finished_at) : "—"}</td>
                    <td>{run.exit_code ?? "—"}</td>
                    <td>
                      {run.retention_expires_at != null ? (
                        <span className="cell-meta">{fmtRetention(nowSecs, run.retention_expires_at)}</span>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td>
                      <button
                        type="button"
                        className="act-btn logs"
                        onClick={() => setSelected(isSelected ? undefined : run.workbench)}
                      >
                        {isSelected ? "HIDE LOG" : "VIEW LOG"}
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {selected !== undefined && (
        <LogViewer
          content={logQuery.data}
          loading={logQuery.isLoading}
          err={logQuery.isError ? logQuery.error.message : undefined}
        />
      )}
    </main>
  );
}

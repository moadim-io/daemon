import { Link, useNavigate, useParams } from "react-router-dom";
import { useRoutine, useRoutineRuns, useRunLog, useRunSummary } from "../../api/hooks";
import { fmtFreshness } from "../../components/RefreshControl";
import { abstime, reltime } from "../../lib/cronUtils";
import { fmtRetention, fmtRunDuration, runStatusClass, runStatusLabel } from "../../lib/runDisplay";
import { useNow } from "../../lib/useNow";
import { AgentRunSummary } from "../routines/AgentRunSummary";
import { LogViewer } from "../routines/LogViewer";

/**
 * A single run's own addressable page: `/runs/:routineId/:workbench`. Deep-linkable
 * and shareable — resolves the routine's title itself via `useRoutine`, so it works
 * standalone from a bookmark or a shared link, not only via in-app navigation from
 * the fleet Runs page or a routine's history table. Ships the run's full log
 * (existing `LogViewer`, already has search + auto-tail) plus Prev/Next hops to its
 * neighbors in the same routine's run history.
 */
export function RunDetailPage() {
  const { routineId = "", workbench = "" } = useParams();
  const navigate = useNavigate();
  const now = useNow();
  const nowSecs = Math.floor(now / 1000);

  const routineQuery = useRoutine(routineId);
  const runsQuery = useRoutineRuns(routineId);
  const logQuery = useRunLog(routineId, workbench);
  const summaryQuery = useRunSummary(routineId, workbench);

  const runs = runsQuery.data ?? [];
  const index = runs.findIndex((r) => r.workbench === workbench);
  const run = index >= 0 ? runs[index] : undefined;
  // Runs are listed newest-first, so the older neighbor sits at the next index and
  // the newer one at the previous index.
  const older = index >= 0 ? runs[index + 1] : undefined;
  const newer = index >= 0 ? runs[index - 1] : undefined;

  const title = routineQuery.data?.title ?? routineId;

  return (
    <main className="logs-page">
      <div className="page-hd">
        <Link to={`/routines?history=${encodeURIComponent(routineId)}`} className="btn btn-ghost btn-sm">
          ← {title}
        </Link>
        <div className="page-title">RUN / {workbench}</div>
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
      ) : run === undefined ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⧗</div>
            <div className="empty-msg">RUN NOT FOUND</div>
            <div className="empty-sub">it may have been reaped, or the link is stale</div>
          </div>
        </div>
      ) : (
        <>
          <div className="stats">
            <div className="stat-card">
              <div className="stat-label">STATUS</div>
              <span className={runStatusClass(run.status)}>{runStatusLabel(run.status)}</span>
            </div>
            <div className="stat-card">
              <div className="stat-label">STARTED</div>
              <div className="stat-val stat-val-sm" title={abstime(run.started_at)}>
                {reltime(run.started_at)}
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-label">DURATION</div>
              <div className="stat-val stat-val-sm">
                {run.finished_at != null ? fmtRunDuration(run.started_at, run.finished_at) : "—"}
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-label">EXIT CODE</div>
              <div className="stat-val stat-val-sm">{run.exit_code ?? "—"}</div>
            </div>
            <div className="stat-card">
              <div className="stat-label">RETENTION</div>
              <div className="stat-val stat-val-sm">
                {run.retention_expires_at != null ? fmtRetention(nowSecs, run.retention_expires_at) : "—"}
              </div>
            </div>
          </div>

          <div className="section-acts">
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              disabled={older === undefined}
              onClick={() => older && navigate(`/runs/${routineId}/${older.workbench}`)}
            >
              ← Older run
            </button>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              disabled={newer === undefined}
              onClick={() => newer && navigate(`/runs/${routineId}/${newer.workbench}`)}
            >
              Newer run →
            </button>
          </div>

          <AgentRunSummary
            content={summaryQuery.data}
            loading={summaryQuery.isLoading}
            err={summaryQuery.isError ? summaryQuery.error.message : undefined}
          />

          <LogViewer
            content={logQuery.data}
            loading={logQuery.isLoading}
            err={logQuery.isError ? logQuery.error.message : undefined}
          />
        </>
      )}
    </main>
  );
}

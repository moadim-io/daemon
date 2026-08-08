import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useAllRuns } from "../../api/hooks";
import { abstime, reltime } from "../../lib/cronUtils";
import { fmtRunDuration, runStatusClass, runStatusLabel } from "../../lib/runDisplay";
import { useNow } from "../../lib/useNow";
import { RefreshFreshness, refreshMs, useRefreshToken } from "../../components/RefreshControl";
import { DEFAULT_RUNS_FILTER, filterRuns, statusCounts, type RunStatusFacet, type RunTimeFacet } from "./runsFilter";

const INITIAL_LIMIT = 100;
const LOAD_MORE_STEP = 100;
const MAX_LIMIT = 1_000;

const STATUS_OPTIONS: [RunStatusFacet, string][] = [
  ["all", "All"],
  ["success", "Success"],
  ["failed", "Failed"],
  ["running", "Running"],
  ["unknown", "Unknown"],
];

const TIME_OPTIONS: [RunTimeFacet, string][] = [
  ["all", "All time"],
  ["1h", "Last hour"],
  ["24h", "Last 24h"],
  ["7d", "Last 7 days"],
];

/**
 * The RUNS page: a fleet-wide, filterable activity feed of every past run
 * across every routine — the build-history list CI dashboards (GitHub
 * Actions, CircleCI) and cron managers ship, so an operator can answer
 * "what ran, and how did it go" without opening each routine's own history
 * or scrolling Overview's fixed 8-row recent-runs panel. Built entirely on
 * the existing `GET /routines/runs` fleet endpoint.
 */
export function RunsPage() {
  const refreshToken = useRefreshToken();
  const ms = refreshMs(refreshToken);
  const nowSecs = Math.floor(useNow(30_000) / 1000);

  const [limit, setLimit] = useState(INITIAL_LIMIT);
  const [filter, setFilter] = useState(DEFAULT_RUNS_FILTER);

  const runsQuery = useAllRuns(limit, { refetchInterval: ms });
  const runs = useMemo(() => runsQuery.data ?? [], [runsQuery.data]);
  const shown = useMemo(() => filterRuns(runs, filter, nowSecs), [runs, filter, nowSecs]);
  const counts = useMemo(() => statusCounts(runs), [runs]);

  const canLoadMore = runs.length >= limit && limit < MAX_LIMIT;

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Runs</h1>
        <div className="section-acts">
          <RefreshFreshness updatedAtMs={runsQuery.dataUpdatedAt} />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">FETCHED</div>
          <div className="stat-val">{runs.length}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">RUNNING</div>
          <div className={counts.running > 0 ? "stat-val c-accent" : "stat-val"}>{counts.running}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">SUCCESS</div>
          <div className="stat-val">{counts.success}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">FAILED</div>
          <div className={counts.failed > 0 ? "stat-val c-red" : "stat-val"}>{counts.failed}</div>
        </div>
      </div>

      <div className="filter-bar">
        <div className="filter-field">
          <input
            type="text"
            className="filter-input"
            placeholder="Search runs by routine…"
            aria-label="Search runs"
            value={filter.query}
            onChange={(e) => setFilter({ ...filter, query: e.target.value })}
          />

          <span className="filter-label">STATUS</span>
          <select
            className="filter-select"
            aria-label="Status filter"
            value={filter.status}
            onChange={(e) => setFilter({ ...filter, status: e.target.value as RunStatusFacet })}
          >
            {STATUS_OPTIONS.map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>

          <span className="filter-label">WHEN</span>
          <select
            className="filter-select"
            aria-label="Time range filter"
            value={filter.time}
            onChange={(e) => setFilter({ ...filter, time: e.target.value as RunTimeFacet })}
          >
            {TIME_OPTIONS.map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </div>
        <div className="filter-field">
          <span className="filter-count">
            Showing {shown.length} of {runs.length} fetched
          </span>
        </div>
      </div>

      {runsQuery.error ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⚠</div>
            <div className="empty-msg">FAILED TO LOAD</div>
            <div className="empty-sub">{runsQuery.error.message}</div>
          </div>
        </div>
      ) : runsQuery.isLoading ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="spinner" />
          </div>
        </div>
      ) : shown.length === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⧗</div>
            <div className="empty-msg">NO RUNS</div>
            <div className="empty-sub">
              {runs.length === 0 ? "no runs yet" : "no runs match the current filters"}
            </div>
          </div>
        </div>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>ROUTINE</th>
                <th>STARTED</th>
                <th>DURATION</th>
                <th>STATUS</th>
                <th>EXIT CODE</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {shown.map((run) => (
                <tr key={run.workbench}>
                  <td>
                    <Link to={`/routines?history=${encodeURIComponent(run.routine_id)}`}>{run.routine_title}</Link>
                  </td>
                  <td>
                    <div className="cell-time" title={abstime(run.started_at)}>
                      {reltime(run.started_at)}
                    </div>
                  </td>
                  <td>
                    <span className="cell-meta">
                      {run.finished_at == null ? "—" : fmtRunDuration(run.started_at, run.finished_at)}
                    </span>
                  </td>
                  <td>
                    <span className={runStatusClass(run.status)}>{runStatusLabel(run.status)}</span>
                  </td>
                  <td>{run.exit_code == null ? "—" : run.exit_code}</td>
                  <td>
                    <Link
                      to={`/runs/${encodeURIComponent(run.routine_id)}/${encodeURIComponent(run.workbench)}`}
                      className="act-btn logs"
                    >
                      VIEW RUN
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {canLoadMore && (
        <div className="section-acts">
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => setLimit((n) => Math.min(MAX_LIMIT, n + LOAD_MORE_STEP))}
          >
            Load more
          </button>
        </div>
      )}
    </div>
  );
}

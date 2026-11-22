import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useAllRuns, useRoutines } from "../../api/hooks";
import { SortableTh } from "../../components/SortableTh";
import { fmtRunDuration, runStatusClass, runStatusLabel } from "../../lib/runDisplay";
import { sortRows, usePersistedSort, type SortAccessors } from "../../lib/tableSort";
import { RefreshFreshness, refreshMs, useRefreshToken } from "../../components/RefreshControl";
import { rateClass, rateLabel } from "../reliability/reliabilityStats";
import {
  computeRepoStats,
  matchesRepoQuery,
  repoSuccessRate,
  routinesWithoutRepo,
  type RepoStats,
} from "./reposStats";

/** Fleet-wide runs fetched to build each repository's recent success rate. Mirrors the Reliability page's cap. */
const FETCH_LIMIT = 300;

const REPO_SORT_KEYS = ["repo", "routines", "enabled", "running", "failing", "rate", "p50"] as const;
type RepoSortKey = (typeof REPO_SORT_KEYS)[number];

const REPO_SORT: SortAccessors<RepoStats, RepoSortKey> = {
  repo: (r) => r.name,
  routines: (r) => r.total,
  enabled: (r) => r.enabled,
  running: (r) => r.runningNow,
  failing: (r) => r.failing,
  rate: (r) => repoSuccessRate(r),
  p50: (r) => r.p50Secs,
};

/**
 * The REPOSITORIES page: a per-repository breakdown joining every routine's `repositories` list
 * against the recent runs — "which repositories does the fleet touch, is any of them failing, is
 * one auto-pulled off" in one table. The component view software catalogs ship, built from data
 * the UI already fetches.
 */
export function ReposPage() {
  const refreshToken = useRefreshToken();
  const ms = refreshMs(refreshToken);

  const routinesQuery = useRoutines({}, { refetchInterval: ms });
  const runsQuery = useAllRuns(FETCH_LIMIT, { refetchInterval: ms });
  const { sort, toggle } = usePersistedSort("moadim.client.repos.sort", REPO_SORT_KEYS);
  const [query, setQuery] = useState("");

  const stats = useMemo(
    () => computeRepoStats(routinesQuery.data ?? [], runsQuery.data ?? []),
    [routinesQuery.data, runsQuery.data],
  );
  const shown = sortRows(
    stats.filter((r) => matchesRepoQuery(r, query)),
    sort,
    REPO_SORT,
  );

  const isLoading = routinesQuery.isLoading || runsQuery.isLoading;
  const errorMessage = routinesQuery.error?.message ?? runsQuery.error?.message;
  const updatedAtMs = Math.min(routinesQuery.dataUpdatedAt || Infinity, runsQuery.dataUpdatedAt || Infinity);

  const withFailing = stats.filter((r) => r.failing > 0).length;

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Repositories</h1>
        <div className="section-acts">
          <RefreshFreshness updatedAtMs={Number.isFinite(updatedAtMs) ? updatedAtMs : 0} />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">REPOSITORIES</div>
          <div className="stat-val">{stats.length}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">WITH FAILING ROUTINES</div>
          <div className={withFailing > 0 ? "stat-val c-red" : "stat-val"}>{withFailing}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">ROUTINES WITHOUT A REPOSITORY</div>
          <div className="stat-val">{routinesWithoutRepo(routinesQuery.data ?? [])}</div>
        </div>
      </div>

      {errorMessage ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⚠</div>
            <div className="empty-msg">FAILED TO LOAD</div>
            <div className="empty-sub">{errorMessage}</div>
          </div>
        </div>
      ) : isLoading ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="spinner" />
          </div>
        </div>
      ) : stats.length === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">📦</div>
            <div className="empty-msg">NO REPOSITORIES YET</div>
            <div className="empty-sub">no routine declares a repository</div>
          </div>
        </div>
      ) : (
        <>
          <div className="filter-bar">
            <input
              type="search"
              className="search-input"
              placeholder="Search repositories…"
              aria-label="Search repositories"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <div className="table-wrap">
            {shown.length === 0 ? (
              <div className="empty">
                <div className="empty-msg">NO MATCHING REPOSITORIES</div>
              </div>
            ) : (
              <table>
                <thead>
                  <tr>
                    <SortableTh label="REPOSITORY" col="repo" sort={sort} defaultDir="asc" onSort={toggle} />
                    <SortableTh label="ROUTINES" col="routines" sort={sort} defaultDir="desc" onSort={toggle} />
                    <SortableTh label="ENABLED" col="enabled" sort={sort} defaultDir="desc" onSort={toggle} />
                    <SortableTh label="RUNNING NOW" col="running" sort={sort} defaultDir="desc" onSort={toggle} />
                    <SortableTh label="FAILING" col="failing" sort={sort} defaultDir="desc" onSort={toggle} />
                    <th>BRANCHES</th>
                    <th>AGENTS</th>
                    <SortableTh label="SUCCESS RATE" col="rate" sort={sort} defaultDir="asc" onSort={toggle} />
                    <SortableTh label="P50" col="p50" sort={sort} defaultDir="desc" onSort={toggle} />
                    <th>LAST ACTIVITY</th>
                  </tr>
                </thead>
                <tbody>
                  {shown.map((r) => (
                    <RepoRow key={r.url} r={r} />
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function RepoRow({ r }: { r: RepoStats }) {
  const rate = repoSuccessRate(r);
  return (
    <tr>
      <td>
        <Link to={`/routines?repo=${encodeURIComponent(r.url)}`} title={r.url}>
          {r.name}
        </Link>
        {r.autoPullOff > 0 ? (
          <span className="run-status running" title="Routines that do not fetch this repository before each run">
            {" "}
            AUTO-PULL OFF ×{r.autoPullOff}
          </span>
        ) : null}
        <details>
          <summary className="cell-meta">
            {r.routines.length} routine{r.routines.length === 1 ? "" : "s"}
          </summary>
          <ul>
            {r.routines.map((rt) => (
              <li key={rt.id}>
                <Link to={`/routines/${encodeURIComponent(rt.id)}`}>{rt.title}</Link>
              </li>
            ))}
          </ul>
        </details>
      </td>
      <td>{r.total}</td>
      <td>{r.enabled}</td>
      <td className={r.runningNow > 0 ? "c-accent" : undefined}>{r.runningNow}</td>
      <td className={r.failing > 0 ? "c-red" : undefined}>{r.failing}</td>
      <td className="cell-meta">{r.branches.join(", ")}</td>
      <td className="cell-meta">{r.agents.join(", ")}</td>
      <td>
        <span className={rateClass(rate)}>{rateLabel(rate)}</span>
      </td>
      <td className="cell-meta">{r.p50Secs === null ? "—" : fmtRunDuration(0, r.p50Secs)}</td>
      <td>
        {r.lastRun ? (
          <>
            <span className={runStatusClass(r.lastRun.status)}>{runStatusLabel(r.lastRun.status)}</span>{" "}
            <span className="cell-meta">{r.lastRun.label}</span>
          </>
        ) : (
          <span className="cell-meta">—</span>
        )}
      </td>
    </tr>
  );
}

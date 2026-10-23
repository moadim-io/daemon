import { useMemo } from "react";
import { Link } from "react-router-dom";
import { useAgents, useAllRuns, useRoutines } from "../../api/hooks";
import { fmtRunDuration, runStatusClass, runStatusLabel } from "../../lib/runDisplay";
import { RefreshFreshness, refreshMs, useRefreshToken } from "../../components/RefreshControl";
import { rateClass, rateLabel } from "../reliability/reliabilityStats";
import { agentSuccessRate, computeAgentStats, healthBadge, isUnhealthy, type AgentStats } from "./agentsStats";

/** Fleet-wide runs fetched to build each agent's recent success rate. Mirrors the Reliability page's cap. */
const FETCH_LIMIT = 300;

/**
 * The AGENTS page: a per-agent breakdown joining the agent registry against every routine's
 * `agent` and recent runs — "which agents does the fleet lean on, is any of them broken on this
 * host, is one failing or slower than the rest" in one table. The executor-pool view CI
 * dashboards ship (runner labels, agent queues), built from data the UI already fetches.
 */
export function AgentsPage() {
  const refreshToken = useRefreshToken();
  const ms = refreshMs(refreshToken);

  const agentsQuery = useAgents();
  const routinesQuery = useRoutines({}, { refetchInterval: ms });
  const runsQuery = useAllRuns(FETCH_LIMIT, { refetchInterval: ms });

  const stats = useMemo(
    () => computeAgentStats(agentsQuery.data ?? [], routinesQuery.data ?? [], runsQuery.data ?? []),
    [agentsQuery.data, routinesQuery.data, runsQuery.data],
  );

  const isLoading = agentsQuery.isLoading || routinesQuery.isLoading || runsQuery.isLoading;
  const errorMessage = agentsQuery.error?.message ?? routinesQuery.error?.message ?? runsQuery.error?.message;
  const updatedAtMs = Math.min(
    agentsQuery.dataUpdatedAt || Infinity,
    routinesQuery.dataUpdatedAt || Infinity,
    runsQuery.dataUpdatedAt || Infinity,
  );

  const unhealthy = stats.filter((a) => isUnhealthy(a.health));
  const blockedRoutines = unhealthy.reduce((n, a) => n + a.enabled, 0);

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Agents</h1>
        <div className="section-acts">
          <RefreshFreshness updatedAtMs={Number.isFinite(updatedAtMs) ? updatedAtMs : 0} />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">AGENTS</div>
          <div className="stat-val">{stats.length}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">UNHEALTHY</div>
          <div className={unhealthy.length > 0 ? "stat-val c-red" : "stat-val"}>{unhealthy.length}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">ENABLED ROUTINES BLOCKED</div>
          <div className={blockedRoutines > 0 ? "stat-val c-red" : "stat-val"}>{blockedRoutines}</div>
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
            <div className="empty-icon">🤖</div>
            <div className="empty-msg">NO AGENTS YET</div>
            <div className="empty-sub">the agent registry is empty and no routine targets an agent</div>
          </div>
        </div>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>AGENT</th>
                <th>HEALTH</th>
                <th>ROUTINES</th>
                <th>ENABLED</th>
                <th>RUNNING NOW</th>
                <th>MODELS</th>
                <th>SUCCESS RATE</th>
                <th>P50</th>
                <th>LAST ACTIVITY</th>
              </tr>
            </thead>
            <tbody>
              {stats.map((a) => (
                <AgentRow key={a.name} a={a} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function AgentRow({ a }: { a: AgentStats }) {
  const badge = healthBadge(a.health);
  return (
    <tr>
      <td>
        {a.total > 0 ? <Link to={`/routines?agent=${encodeURIComponent(a.name)}`}>{a.name}</Link> : a.name}
        {a.routines.length > 0 ? (
          <details>
            <summary className="cell-meta">{a.routines.length} routine{a.routines.length === 1 ? "" : "s"}</summary>
            <ul>
              {a.routines.map((r) => (
                <li key={r.id}>
                  <Link to={`/routines?history=${encodeURIComponent(r.id)}`}>{r.title}</Link>
                </li>
              ))}
            </ul>
          </details>
        ) : null}
      </td>
      <td>
        <span className={badge.className} title={badge.title}>
          {badge.label}
        </span>
      </td>
      <td>{a.total}</td>
      <td>{a.enabled}</td>
      <td className={a.runningNow > 0 ? "c-accent" : undefined}>{a.runningNow}</td>
      <td className="cell-meta">{a.models.length > 0 ? a.models.join(", ") : "default"}</td>
      <td>
        <span className={rateClass(agentSuccessRate(a))}>{rateLabel(agentSuccessRate(a))}</span>
      </td>
      <td className="cell-meta">{a.p50Secs === null ? "—" : fmtRunDuration(0, a.p50Secs)}</td>
      <td>
        {a.lastRun ? (
          <>
            <span className={runStatusClass(a.lastRun.status)}>{runStatusLabel(a.lastRun.status)}</span>{" "}
            <span className="cell-meta">{a.lastRun.label}</span>
          </>
        ) : (
          <span className="cell-meta">—</span>
        )}
      </td>
    </tr>
  );
}

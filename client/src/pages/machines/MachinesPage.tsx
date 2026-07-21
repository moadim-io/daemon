import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useAllRuns, useMachine, useMachines, useRoutines } from "../../api/hooks";
import { runStatusClass, runStatusLabel } from "../../lib/runDisplay";
import {
  loadRefreshToken,
  refreshMs,
  RefreshControl,
  saveRefreshToken,
  type RefreshToken,
} from "../../components/RefreshControl";
import { distinctMachines } from "../routines/filter";
import { rateClass, rateLabel } from "../reliability/reliabilityStats";
import {
  computeMachineStats,
  machineSuccessRate,
  unassignedRoutineCount,
  type MachineStats,
} from "./machinesStats";

/** Fleet-wide runs fetched to build each machine's recent success rate. Mirrors the Reliability page's cap. */
const FETCH_LIMIT = 300;

/**
 * The MACHINES page: a fleet inventory joining the known machine names against every routine's
 * targeting list and recent runs — "how many routines does each host run, is anything live or
 * broken there, how healthy is it lately" without cross-referencing the Routines table by hand.
 * The runner/host-pool view most CI and cron-manager dashboards ship (GitHub Actions "Runners",
 * Jenkins "Nodes"), built entirely from data the UI already fetches elsewhere.
 */
export function MachinesPage() {
  const [refreshToken, setRefreshToken] = useState<RefreshToken>(loadRefreshToken);
  const ms = refreshMs(refreshToken);

  const machinesQuery = useMachines();
  const currentMachineQuery = useMachine();
  const routinesQuery = useRoutines({}, { refetchInterval: ms });
  const runsQuery = useAllRuns(FETCH_LIMIT, { refetchInterval: ms });

  const onChangeRefresh = (next: RefreshToken) => {
    saveRefreshToken(next);
    setRefreshToken(next);
  };

  const routines = useMemo(() => routinesQuery.data ?? [], [routinesQuery.data]);
  const runs = useMemo(() => runsQuery.data ?? [], [runsQuery.data]);

  const names = useMemo(
    () => [...new Set([...(machinesQuery.data ?? []), ...distinctMachines(routines)])].sort(),
    [machinesQuery.data, routines],
  );

  const stats = useMemo(
    () => computeMachineStats(names, routines, runs, currentMachineQuery.data?.name, new Date()),
    [names, routines, runs, currentMachineQuery.data?.name],
  );

  const isLoading = machinesQuery.isLoading || routinesQuery.isLoading || runsQuery.isLoading;
  const errorMessage = machinesQuery.error?.message ?? routinesQuery.error?.message ?? runsQuery.error?.message;
  const updatedAtMs = Math.min(
    machinesQuery.dataUpdatedAt || Infinity,
    routinesQuery.dataUpdatedAt || Infinity,
    runsQuery.dataUpdatedAt || Infinity,
  );

  const unassigned = unassignedRoutineCount(routines);
  const runningNow = stats.reduce((n, m) => n + m.runningNow, 0);

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Machines</h1>
        <div className="section-acts">
          <RefreshControl
            token={refreshToken}
            updatedAtMs={Number.isFinite(updatedAtMs) ? updatedAtMs : 0}
            onChange={onChangeRefresh}
          />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">MACHINES</div>
          <div className="stat-val">{names.length}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">RUNNING NOW</div>
          <div className={runningNow > 0 ? "stat-val c-accent" : "stat-val"}>{runningNow}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">UNASSIGNED ROUTINES</div>
          <div className={unassigned > 0 ? "stat-val c-amber" : "stat-val"}>{unassigned}</div>
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
            <div className="empty-icon">🖥</div>
            <div className="empty-msg">NO MACHINES YET</div>
            <div className="empty-sub">no routine targets a machine, and this daemon has no resolved identity</div>
          </div>
        </div>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>MACHINE</th>
                <th>ROUTINES</th>
                <th>ENABLED</th>
                <th>RUNNING NOW</th>
                <th>POWER SAVING</th>
                <th>ATTENTION</th>
                <th>SUCCESS RATE</th>
                <th>LAST ACTIVITY</th>
              </tr>
            </thead>
            <tbody>
              {stats.map((m) => (
                <MachineRow key={m.name} m={m} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function MachineRow({ m }: { m: MachineStats }) {
  return (
    <tr>
      <td>
        {m.name}
        {m.isCurrent ? <span className="cell-meta"> (this machine)</span> : null}
        {m.routines.length > 0 ? (
          <details>
            <summary className="cell-meta">{m.routines.length} routine{m.routines.length === 1 ? "" : "s"}</summary>
            <ul>
              {m.routines.map((r) => (
                <li key={r.id}>
                  <Link to={`/routines?history=${encodeURIComponent(r.id)}`}>{r.title}</Link>
                </li>
              ))}
            </ul>
          </details>
        ) : null}
      </td>
      <td>{m.total}</td>
      <td>{m.enabled}</td>
      <td className={m.runningNow > 0 ? "c-accent" : undefined}>{m.runningNow}</td>
      <td>{m.powerSaving}</td>
      <td className={m.needsAttention > 0 ? "c-red" : "cell-meta"}>{m.needsAttention || "—"}</td>
      <td>
        <span className={rateClass(machineSuccessRate(m))}>{rateLabel(machineSuccessRate(m))}</span>
      </td>
      <td>
        {m.lastRun ? (
          <>
            <span className={runStatusClass(m.lastRun.status)}>{runStatusLabel(m.lastRun.status)}</span>{" "}
            <span className="cell-meta">{m.lastRun.label}</span>
          </>
        ) : (
          <span className="cell-meta">—</span>
        )}
      </td>
    </tr>
  );
}

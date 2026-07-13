import { Link } from "react-router-dom";
import type { FleetRunSummary } from "../../api/hooks";
import { reltime } from "../../lib/cronUtils";
import { fmtRunDuration, runStatusClass, runStatusLabel } from "../../lib/runDisplay";

/**
 * The Overview page's fleet-wide "recent runs" table: the most recent runs
 * across every routine, complementing `UpcomingTable`'s future-fire view
 * with the equivalent view of the past. Direct port of
 * `ui/src/overview_recent_runs.rs`.
 */
export function RecentRunsTable({ runs, loading }: { runs: FleetRunSummary[]; loading: boolean }) {
  if (loading) {
    return (
      <div className="table-wrap">
        <div className="empty">
          <div className="spinner" />
        </div>
      </div>
    );
  }
  if (runs.length === 0) {
    return (
      <div className="table-wrap">
        <div className="empty">
          <div className="empty-icon">⧗</div>
          <div className="empty-msg">NO RUNS YET</div>
        </div>
      </div>
    );
  }

  return (
    <div className="table-wrap">
      <table>
        <thead>
          <tr>
            <th>ROUTINE</th>
            <th>STARTED</th>
            <th>DURATION</th>
            <th>STATUS</th>
            <th>EXIT CODE</th>
          </tr>
        </thead>
        <tbody>
          {runs.map((run) => (
            <tr key={run.workbench}>
              <td>
                <Link to={`/routines?history=${encodeURIComponent(run.routine_id)}`}>{run.routine_title}</Link>
              </td>
              <td>
                <div className="cell-time">{reltime(run.started_at)}</div>
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
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

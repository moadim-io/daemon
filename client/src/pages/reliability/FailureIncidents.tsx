import { Link } from "react-router-dom";
import { abstime } from "../../lib/cronUtils";
import { fmtRunDuration } from "../../lib/runDisplay";
import type { IncidentReport } from "./incidentMath";

function fmtSecs(secs: number | null): string {
  return secs === null ? "—" : fmtRunDuration(0, secs);
}

/**
 * Failure incidents: each routine's consecutive failed runs as one episode with its time to
 * recover, ongoing ones first, and the fleet MTTR — how long things stay broken, which success
 * rates and streaks don't show.
 */
export function FailureIncidents({ report }: { report: IncidentReport }) {
  const { incidents, openCount, mttrSecs, longestSecs } = report;
  if (incidents.length === 0) return null;
  return (
    <details className="table-wrap adherence" open>
      <summary className="adherence-hd">
        <span className="filter-label">FAILURE INCIDENTS</span>
        <span className="cell-meta">
          <span className={openCount > 0 ? "c-red" : undefined}>{openCount} open</span> · {incidents.length} total ·
          MTTR {fmtSecs(mttrSecs)} · longest {fmtSecs(longestSecs)}
        </span>
      </summary>
      <p className="adherence-note">
        Consecutive failed runs of a routine form one incident, resolved when its next run succeeds. ≥ marks an
        incident that began before the oldest run in the fetched sample.
      </p>
      <table>
        <thead>
          <tr>
            <th>ROUTINE</th>
            <th>STATUS</th>
            <th>OPENED</th>
            <th>RESOLVED</th>
            <th>TIME TO RECOVER</th>
            <th>FAILED RUNS</th>
          </tr>
        </thead>
        <tbody>
          {incidents.map((inc) => {
            const ge = inc.truncated ? "≥ " : "";
            return (
              <tr key={`${inc.routineId}-${inc.openedAt}`}>
                <td>
                  <Link to={`/routines?history=${encodeURIComponent(inc.routineId)}`}>{inc.routineTitle}</Link>
                </td>
                <td>
                  {inc.resolvedAt === null ? (
                    <span className="run-status failed">ONGOING</span>
                  ) : (
                    <span className="run-status success">RESOLVED</span>
                  )}
                </td>
                <td className="cell-meta">
                  <Link
                    to={`/runs/${encodeURIComponent(inc.routineId)}/${encodeURIComponent(inc.firstWorkbench)}`}
                    title="Open the first failed run"
                  >
                    {ge}
                    {abstime(inc.openedAt)}
                  </Link>
                </td>
                <td className="cell-meta">{inc.resolvedAt === null ? "—" : abstime(inc.resolvedAt)}</td>
                <td className={inc.resolvedAt === null ? "c-red" : "cell-meta"}>
                  {ge}
                  {fmtSecs(inc.durationSecs)}
                </td>
                <td className="cell-meta">
                  {ge}
                  {inc.failedRuns}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </details>
  );
}

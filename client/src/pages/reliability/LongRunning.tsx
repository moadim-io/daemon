import { Link } from "react-router-dom";
import { fmtRunDuration } from "../../lib/runDisplay";
import { OVERRUN_FACTOR, type OverrunReport } from "./overrunMath";

/**
 * Long-running now: live runs that have already outlasted their routine's own p95 by
 * `OVERRUN_FACTOR`, worst first — the earliest signal of a hung agent.
 */
export function LongRunning({ report }: { report: OverrunReport }) {
  const { overruns, runningCount, noBaselineCount } = report;
  if (runningCount === 0) return null;
  return (
    <details className="table-wrap adherence" open={overruns.length > 0}>
      <summary className="adherence-hd">
        <span className="filter-label">LONG-RUNNING NOW</span>
        <span className="cell-meta">
          {overruns.length} of {runningCount} running over baseline
          {noBaselineCount > 0 ? ` · ${noBaselineCount} without baseline` : ""}
        </span>
      </summary>
      <p className="adherence-note">
        Live runs whose elapsed time exceeds {OVERRUN_FACTOR}× their routine&apos;s p95 duration. A large overrun
        usually means a stuck agent.
      </p>
      {overruns.length > 0 ? (
        <table>
          <thead>
            <tr>
              <th>ROUTINE</th>
              <th>ELAPSED</th>
              <th>P95</th>
              <th>OVERRUN</th>
            </tr>
          </thead>
          <tbody>
            {overruns.map((o) => (
              <tr key={`${o.routineId}/${o.workbench}`}>
                <td>
                  <Link
                    to={`/runs/${encodeURIComponent(o.routineId)}/${encodeURIComponent(o.workbench)}`}
                    title="Open the live run"
                  >
                    {o.routineTitle}
                  </Link>
                </td>
                <td className="cell-meta">{fmtRunDuration(0, o.elapsedSecs)}</td>
                <td className="cell-meta">{fmtRunDuration(0, o.p95Secs)}</td>
                <td>
                  <span className={o.ratio >= OVERRUN_FACTOR * 2 ? "run-status failed" : "run-status running"}>
                    {o.ratio.toFixed(1)}×
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </details>
  );
}

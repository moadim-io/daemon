import { Link } from "react-router-dom";
import { fmtUntil, fmtWhen } from "../../lib/schedule";
import type { UpcomingRun } from "./overviewLogic";

export interface UpcomingTableProps {
  runs: UpcomingRun[];
  now: Date;
  loading: boolean;
  error: string | undefined;
  onTrigger: (id: string) => void;
}

/**
 * The Overview page's "upcoming runs" table with a trigger button per row.
 * Direct port of `ui/src/overview_upcoming.rs (removed)`.
 */
export function UpcomingTable({ runs, now, loading, error, onTrigger }: UpcomingTableProps) {
  if (error !== undefined) {
    return (
      <div className="table-wrap">
        <div className="empty">
          <div className="empty-icon">⚠</div>
          <div className="empty-msg">FAILED TO LOAD</div>
          <div className="empty-sub">{error}</div>
        </div>
      </div>
    );
  }
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
          <div className="empty-icon">◷</div>
          <div className="empty-msg">NO UPCOMING RUNS</div>
          <div className="empty-sub">no enabled routine is scheduled to fire</div>
        </div>
      </div>
    );
  }

  return (
    <div className="table-wrap">
      <table>
        <thead>
          <tr>
            <th>TYPE</th>
            <th>NAME</th>
            <th>SCHEDULE</th>
            <th>NEXT RUN</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {runs.map((run, i) => (
            <tr key={i}>
              <td>
                <span className="kind-badge routine">ROUTINE</span>
              </td>
              <td>
                <Link className="ov-name-link" to="/routines">
                  {run.label}
                </Link>
                {run.flagCount > 0 && (
                  <span
                    className="ov-flag-badge"
                    title={`${run.flagCount} open flag${run.flagCount === 1 ? "" : "s"}`}
                  >
                    {`⚑ ${run.flagCount}`}
                  </span>
                )}
              </td>
              <td>
                <div className="cell-schedule-human">{run.human ?? run.schedule}</div>
              </td>
              <td className="cell-next">
                <div className="cell-next-when">{fmtWhen(now, run.at)}</div>
                <div className={run.soon ? "cell-next-until soon" : "cell-next-until"}>{fmtUntil(now, run.at)}</div>
              </td>
              <td className="cell-act">
                <button className="btn btn-sm btn-ghost run-now-btn" title="Trigger now" onClick={() => onTrigger(run.id)}>
                  ▶ RUN
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

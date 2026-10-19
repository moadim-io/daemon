import { Link } from "react-router-dom";
import { abstime } from "../../lib/cronUtils";
import { fmtRunDuration } from "../../lib/runDisplay";
import { GRACE_SECS, type Adherence } from "./adherenceMath";
import { rateClass, rateLabel } from "./reliabilityStats";

function fmtLag(secs: number | null): string {
  return secs === null ? "—" : fmtRunDuration(0, secs);
}

/**
 * Schedule adherence over the last day: every expected cron fire of the routines assigned to this
 * machine, matched to the run it produced — surfacing fires that never ran (which success rates
 * can't see) and how late the ones that did started.
 */
export function ScheduleAdherence({ adherence }: { adherence: Adherence }) {
  const { rows, expected, ran, missed, medianLagSecs, truncated, from } = adherence;
  if (rows.length === 0) return null;
  const rate = ran / expected;
  const hours = Math.max(1, Math.round((adherence.to - from) / 3_600));
  return (
    <details className="table-wrap adherence" open>
      <summary className="adherence-hd">
        <span className="filter-label">SCHEDULE ADHERENCE · LAST {truncated ? `~${hours}H` : "24H"}</span>
        <span className={missed > 0 ? "c-red" : "cell-meta"}>
          {ran}/{expected} fires ran — {missed} missed · <span className={rateClass(rate)}>{rateLabel(rate)}</span> ·
          median start lag {fmtLag(medianLagSecs)}
        </span>
      </summary>
      <p className="adherence-note">
        A fire counts as run when this routine started within {GRACE_SECS / 60}m of it. Past snoozes and
        power-saving skips aren&apos;t recorded, so they show as missed
        {truncated && "; the window is shortened to the oldest run in the fetched sample"}.
      </p>
      <table>
        <thead>
          <tr>
            <th>ROUTINE</th>
            <th>EXPECTED</th>
            <th>RAN</th>
            <th>MISSED</th>
            <th>ADHERENCE</th>
            <th>MEDIAN LAG</th>
            <th>LAST MISSED</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.routineId}>
              <td>
                <Link to={`/routines?history=${encodeURIComponent(row.routineId)}`}>{row.title}</Link>
              </td>
              <td className="cell-meta">{row.expected}</td>
              <td className="cell-meta">{row.ran}</td>
              <td className={row.missed > 0 ? "c-red" : "cell-meta"}>{row.missed}</td>
              <td>
                <span className={rateClass(row.ran / row.expected)}>{rateLabel(row.ran / row.expected)}</span>
              </td>
              <td className="cell-meta">{fmtLag(row.medianLagSecs)}</td>
              <td className="cell-meta">{row.lastMissedAt === null ? "—" : abstime(row.lastMissedAt)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </details>
  );
}

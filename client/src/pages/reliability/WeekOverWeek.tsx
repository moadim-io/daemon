import { Link } from "react-router-dom";
import { fmtRunDuration } from "../../lib/runDisplay";
import { rateLabel } from "./reliabilityStats";
import { RATE_DELTA_PP, type RoutineChange, type Verdict, type WeekOverWeekReport, type WindowStats } from "./weekOverWeekMath";

const VERDICT_CLASS: Record<Verdict, string> = {
  worse: "run-status failed",
  quiet: "run-status running",
  new: "cell-meta",
  better: "run-status success",
  steady: "cell-meta",
};

function fmtDeltaPp(pp: number | null): string {
  if (pp === null) return "";
  const r = Math.round(pp);
  return r === 0 ? " (±0pp)" : ` (${r > 0 ? "+" : ""}${r}pp)`;
}

function fmtRatio(ratio: number | null): string {
  if (ratio === null) return "";
  const pct = Math.round((ratio - 1) * 100);
  return pct === 0 ? " (±0%)" : ` (${pct > 0 ? "+" : ""}${pct}%)`;
}

function fmtP50(s: WindowStats): string {
  return s.p50Secs === null ? "—" : fmtRunDuration(0, s.p50Secs);
}

function runsCell(s: WindowStats): string {
  return s.failures > 0 ? `${s.runs} · ${s.failures} failed` : `${s.runs}`;
}

/**
 * Week over week: each routine's last 7 days against the 7 days before — success rate and p50
 * deltas, plus routines that went quiet or are new — worst change first.
 */
export function WeekOverWeek({ report }: { report: WeekOverWeekReport }) {
  const { changes, fleetCurrent, fleetPrior, priorPartial } = report;
  if (changes.length === 0) return null;
  const flagged = changes.filter((c) => c.verdict === "worse" || c.verdict === "quiet").length;
  const fleetDelta =
    fleetCurrent.rate === null || fleetPrior.rate === null ? null : (fleetCurrent.rate - fleetPrior.rate) * 100;
  return (
    <details className="table-wrap adherence" open={flagged > 0}>
      <summary className="adherence-hd">
        <span className="filter-label">WEEK OVER WEEK</span>
        <span className="cell-meta">
          fleet {rateLabel(fleetPrior.rate)} → {rateLabel(fleetCurrent.rate)}
          {fmtDeltaPp(fleetDelta)} · {flagged} worse or quiet
          {priorPartial ? " · prior week partial" : ""}
        </span>
      </summary>
      <p className="adherence-note">
        Finished runs in the last 7 days against the 7 days before. WORSE means the success rate fell by{" "}
        {RATE_DELTA_PP}pp or more, or p50 grew by 25% or more. QUIET means it ran last week but not this week.
        {priorPartial ? " The fetched sample does not reach back 14 days, so the prior week is incomplete." : ""}
      </p>
      <table>
        <thead>
          <tr>
            <th>ROUTINE</th>
            <th>CHANGE</th>
            <th>RUNS (PRIOR → NOW)</th>
            <th>SUCCESS RATE</th>
            <th>P50</th>
          </tr>
        </thead>
        <tbody>
          {changes.map((c) => (
            <ChangeRow key={c.routineId} change={c} />
          ))}
        </tbody>
      </table>
    </details>
  );
}

function ChangeRow({ change: c }: { change: RoutineChange }) {
  return (
    <tr>
      <td>
        <Link to={`/routines?history=${encodeURIComponent(c.routineId)}`}>{c.routineTitle}</Link>
      </td>
      <td>
        <span className={VERDICT_CLASS[c.verdict]}>{c.verdict.toUpperCase()}</span>
      </td>
      <td className="cell-meta">
        {runsCell(c.prior)} → {runsCell(c.current)}
      </td>
      <td className="cell-meta">
        {rateLabel(c.prior.rate)} → {rateLabel(c.current.rate)}
        {fmtDeltaPp(c.rateDeltaPp)}
      </td>
      <td className="cell-meta">
        {fmtP50(c.prior)} → {fmtP50(c.current)}
        {fmtRatio(c.p50Ratio)}
      </td>
    </tr>
  );
}

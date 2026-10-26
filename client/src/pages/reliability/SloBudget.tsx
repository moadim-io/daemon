import { Link } from "react-router-dom";
import {
  AT_RISK_LEFT,
  FAST_BURN,
  SLO_TARGETS,
  targetLabel,
  type BudgetStatus,
  type RoutineBudget,
  type SloReport,
} from "./sloMath";

const STATUS_CLASS: Record<BudgetStatus, string> = {
  exhausted: "run-status failed",
  "fast-burn": "run-status failed",
  "at-risk": "run-status running",
  ok: "run-status success",
};

const STATUS_LABEL: Record<BudgetStatus, string> = {
  exhausted: "EXHAUSTED",
  "fast-burn": "FAST BURN",
  "at-risk": "AT RISK",
  ok: "OK",
};

function fmtBurn(burn: number | null): string {
  return burn === null ? "—" : `${burn.toFixed(1)}×`;
}

function fmtAllowed(allowed: number): string {
  return allowed.toFixed(allowed < 10 ? 1 : 0);
}

export interface SloBudgetProps {
  report: SloReport;
  target: number;
  onTargetChange: (target: number) => void;
}

/**
 * SLO error budget: each routine's spent share of the failures a success-rate target allows over
 * the last 7 days, with 24h and 7d burn rates — exhausted and fast-burning routines first.
 */
export function SloBudget({ report, target, onTargetChange }: SloBudgetProps) {
  const { budgets, exhaustedCount, partial } = report;
  if (budgets.length === 0) return null;
  const flagged = budgets.filter((b) => b.status !== "ok").length;
  return (
    <details className="table-wrap adherence" open={flagged > 0}>
      <summary className="adherence-hd">
        <span className="filter-label">ERROR BUDGET</span>
        <span className="cell-meta">
          SLO {targetLabel(target)} · 7d · {exhaustedCount} exhausted · {flagged} need attention
          {partial ? " · window partial" : ""}
        </span>
      </summary>
      <p className="adherence-note">
        <label htmlFor="slo-target-select">SLO TARGET </label>
        <select
          id="slo-target-select"
          className="filter-select"
          value={target}
          onChange={(e) => onTargetChange(Number(e.target.value))}
        >
          {SLO_TARGETS.map((t) => (
            <option key={t} value={t}>
              {targetLabel(t)}
            </option>
          ))}
        </select>{" "}
        The budget is the failures the target allows over finished runs in the last 7 days. Burn rate is the
        failure rate over the allowed rate: 1× spends the budget exactly at the window end. FAST BURN means the
        last 24h burn is {FAST_BURN}× or more. AT RISK means less than {Math.round(AT_RISK_LEFT * 100)}% of the
        budget is left.
        {partial ? " The fetched sample does not reach back 7 days, so the window is incomplete." : ""}
      </p>
      <table>
        <thead>
          <tr>
            <th>ROUTINE</th>
            <th>STATUS</th>
            <th>FAILURES / ALLOWED</th>
            <th>BUDGET LEFT</th>
            <th>BURN 24H</th>
            <th>BURN 7D</th>
          </tr>
        </thead>
        <tbody>
          {budgets.map((b) => (
            <BudgetRow key={b.routineId} budget={b} />
          ))}
        </tbody>
      </table>
    </details>
  );
}

function BudgetRow({ budget: b }: { budget: RoutineBudget }) {
  const leftPct = Math.round(b.budgetLeft * 100);
  return (
    <tr>
      <td>
        <Link to={`/routines?history=${encodeURIComponent(b.routineId)}`}>{b.routineTitle}</Link>
      </td>
      <td>
        <span className={STATUS_CLASS[b.status]}>{STATUS_LABEL[b.status]}</span>
      </td>
      <td className="cell-meta">
        {b.failures} / {fmtAllowed(b.allowed)} of {b.runs} runs
      </td>
      <td className="cell-meta">
        <meter
          min={0}
          max={1}
          low={AT_RISK_LEFT}
          high={0.5}
          optimum={1}
          value={Math.max(0, b.budgetLeft)}
          aria-label={`${b.routineTitle} budget left`}
        />{" "}
        {leftPct}%
      </td>
      <td className="cell-meta">{fmtBurn(b.burn24h)}</td>
      <td className="cell-meta">{fmtBurn(b.burn7d)}</td>
    </tr>
  );
}

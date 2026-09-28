import { Link } from "react-router-dom";
import { abstime } from "../../lib/cronUtils";
import { exitCodeMeaning, type FailureCauseReport } from "./exitCodeMath";

/** Routines listed inline per cause before collapsing the rest into "+N more". */
const MAX_ROUTINES = 3;

/**
 * Failure causes: failed runs grouped by exit code, so a fleet-wide pattern (timeouts, OOM kills,
 * missing binaries) stands out from a single routine's own bug.
 */
export function FailureCauses({ report }: { report: FailureCauseReport }) {
  const { causes, totalFailed } = report;
  if (causes.length === 0) return null;
  return (
    <details className="table-wrap adherence" open>
      <summary className="adherence-hd">
        <span className="filter-label">FAILURE CAUSES</span>
        <span className="cell-meta">
          {totalFailed} failed runs · {causes.length} distinct exit {causes.length === 1 ? "code" : "codes"}
        </span>
      </summary>
      <p className="adherence-note">
        Failed runs in the fetched sample grouped by exit code. The same code across many routines usually means a
        host-level cause.
      </p>
      <table>
        <thead>
          <tr>
            <th>EXIT CODE</th>
            <th>MEANING</th>
            <th>FAILURES</th>
            <th>ROUTINES</th>
            <th>LAST SEEN</th>
          </tr>
        </thead>
        <tbody>
          {causes.map((c) => {
            const extra = c.routines.length - MAX_ROUTINES;
            return (
              <tr key={c.code ?? "none"}>
                <td>
                  <span className="run-status failed">{c.code ?? "?"}</span>
                </td>
                <td className="cell-meta">{exitCodeMeaning(c.code)}</td>
                <td className="cell-meta">
                  {c.count} ({Math.round((c.count / totalFailed) * 100)}%)
                </td>
                <td>
                  {c.routines.slice(0, MAX_ROUTINES).map((r, i) => (
                    <span key={r.id}>
                      {i > 0 ? ", " : null}
                      <Link to={`/routines?history=${encodeURIComponent(r.id)}`}>{r.title}</Link>
                      {r.count > 1 ? <span className="cell-meta"> ×{r.count}</span> : null}
                    </span>
                  ))}
                  {extra > 0 ? <span className="cell-meta"> +{extra} more</span> : null}
                </td>
                <td className="cell-meta">
                  <Link
                    to={`/runs/${encodeURIComponent(c.last.routineId)}/${encodeURIComponent(c.last.workbench)}`}
                    title="Open the newest failed run with this exit code"
                  >
                    {abstime(c.last.at)}
                  </Link>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </details>
  );
}

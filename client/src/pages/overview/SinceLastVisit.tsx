import { Link } from "react-router-dom";
import { abstime, reltime } from "../../lib/cronUtils";
import type { VisitDigest } from "./visitDigest";

export interface SinceLastVisitProps {
  digest: VisitDigest;
  /** Unix seconds of the baseline the digest counts from. */
  sinceSecs: number;
  onMarkSeen: () => void;
}

/**
 * The Overview's SINCE YOUR LAST VISIT digest: what finished while the operator was away —
 * failure/success counts and one row per routine that failed, badged RECOVERED when its
 * newest run has since succeeded. Persists until "Mark as seen" moves the baseline to now.
 */
export function SinceLastVisit({ digest, sinceSecs, onMarkSeen }: SinceLastVisitProps) {
  const empty = digest.failed === 0 && digest.succeeded === 0;
  return (
    <section aria-label="Since your last visit">
      <div className="section-hd">
        <span className={digest.failed > 0 ? "section-label attn" : "section-label"}>SINCE YOUR LAST VISIT</span>
        <div className="section-acts">
          <span className="cell-meta" title={abstime(sinceSecs)}>
            {reltime(sinceSecs)}
          </span>
          {!empty && (
            <button type="button" className="btn btn-sm btn-ghost" onClick={onMarkSeen}>
              Mark as seen
            </button>
          )}
        </div>
      </div>
      {empty ? (
        <p className="cell-meta">No runs finished since your last visit.</p>
      ) : (
        <p className="cell-meta">
          <span className={digest.failed > 0 ? "c-red" : undefined}>{digest.failed} failed</span> ·{" "}
          {digest.succeeded} succeeded
          {digest.truncated && " · older runs not loaded, counts may be low"}
        </p>
      )}
      {digest.routines.length > 0 && (
        <div className="table-wrap attn-wrap">
          <table>
            <thead>
              <tr>
                <th>ROUTINE</th>
                <th>FAILURES</th>
                <th>LATEST FAILURE</th>
                <th>NOW</th>
              </tr>
            </thead>
            <tbody>
              {digest.routines.map((r) => (
                <tr key={r.routineId}>
                  <td>
                    <Link className="ov-name-link" to={`/routines?history=${encodeURIComponent(r.routineId)}`}>
                      {r.routineTitle}
                    </Link>
                  </td>
                  <td>{r.failures}</td>
                  <td>
                    <Link
                      to={`/runs/${encodeURIComponent(r.routineId)}/${encodeURIComponent(r.latestFailure.workbench)}`}
                      title={abstime(r.latestFailure.finished_at ?? 0)}
                    >
                      {reltime(r.latestFailure.finished_at ?? 0)}
                    </Link>
                  </td>
                  <td>
                    {r.recovered ? (
                      <span className="run-status success">RECOVERED</span>
                    ) : (
                      <span className="run-status failed">FAILING</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

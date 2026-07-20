import { useState } from "react";
import { Link } from "react-router-dom";
import { useAllRuns } from "../../api/hooks";
import { fmtRunDuration } from "../../lib/runDisplay";
import {
  loadRefreshToken,
  refreshMs,
  RefreshControl,
  saveRefreshToken,
  type RefreshToken,
} from "../../components/RefreshControl";
import {
  computeReliability,
  fleetSummary,
  isFlaky,
  rateClass,
  rateLabel,
  streakClass,
  streakLabel,
  successRate,
  type RoutineReliability,
} from "./reliabilityStats";

/**
 * Fleet-wide runs fetched to build the reliability sample. Mirrors the Routines table's
 * sparkline fetch cap (`GET /routines/runs` truncates its newest-first merged list to this many
 * total, across every routine) — high enough that an active fleet's routines each keep a
 * `SAMPLE_LEN`-sized window without an unbounded payload.
 */
const FETCH_LIMIT = 300;

function fmtSecs(secs: number | null): string {
  return secs === null ? "—" : fmtRunDuration(0, secs);
}

/**
 * The RELIABILITY page: ranks every routine by recent run outcomes and duration so an operator
 * can spot what's actively broken, flaky, or trending slower without opening each routine's
 * HISTORY tab individually.
 */
export function ReliabilityPage() {
  const [refreshToken, setRefreshToken] = useState<RefreshToken>(loadRefreshToken);
  const {
    data: runs,
    isLoading,
    error,
    dataUpdatedAt,
  } = useAllRuns(FETCH_LIMIT, { refetchInterval: refreshMs(refreshToken) });

  const onChangeRefresh = (next: RefreshToken) => {
    saveRefreshToken(next);
    setRefreshToken(next);
  };

  const items = computeReliability(runs ?? []);
  const summary = fleetSummary(items);
  const fleetRate = summary.sampleSize === 0 ? null : summary.successes / summary.sampleSize;
  const errorMessage = error?.message;

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Reliability</h1>
        <div className="section-acts">
          <RefreshControl token={refreshToken} updatedAtMs={dataUpdatedAt} onChange={onChangeRefresh} />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">FLEET SUCCESS RATE</div>
          <div className="stat-val">{rateLabel(fleetRate)}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">FAILING</div>
          <div className={summary.failingCount > 0 ? "stat-val c-red" : "stat-val"}>{summary.failingCount}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">FLAKY</div>
          <div className={summary.flakyCount > 0 ? "stat-val c-amber" : "stat-val"}>{summary.flakyCount}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">FLEET P50</div>
          <div className="stat-val stat-val-sm">{fmtSecs(summary.p50Secs)}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">FLEET P95</div>
          <div className="stat-val stat-val-sm">{fmtSecs(summary.p95Secs)}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">SLOWER TREND</div>
          <div className={summary.regressingCount > 0 ? "stat-val c-amber" : "stat-val"}>
            {summary.regressingCount}
          </div>
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
      ) : items.length === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">✓</div>
            <div className="empty-msg">NO FINISHED RUNS YET</div>
            <div className="empty-sub">reliability metrics need at least one success or failure</div>
          </div>
        </div>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>ROUTINE</th>
                <th>STREAK</th>
                <th>SUCCESS RATE</th>
                <th>P50</th>
                <th>P95</th>
                <th>TREND</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => (
                <ReliabilityRow key={item.routineId} item={item} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function ReliabilityRow({ item }: { item: RoutineReliability }) {
  return (
    <tr>
      <td>
        <Link to={`/routines?history=${encodeURIComponent(item.routineId)}`}>{item.routineTitle}</Link>
        {isFlaky(item) ? (
          <span className="run-status running" title="Alternating pass/fail over the recent sample">
            {" "}
            FLAKY
          </span>
        ) : null}
      </td>
      <td>
        <span className={streakClass(item.streak)}>{streakLabel(item.streak)}</span>
      </td>
      <td>
        <span className={rateClass(successRate(item))}>{rateLabel(successRate(item))}</span>
      </td>
      <td>
        <span className="cell-meta">{fmtSecs(item.p50Secs)}</span>
      </td>
      <td>
        <span className="cell-meta">{fmtSecs(item.p95Secs)}</span>
      </td>
      <td>
        {item.regressing ? (
          <span className="run-status failed" title="Recent runs are meaningfully slower than the baseline">
            SLOWER
          </span>
        ) : (
          <span className="cell-meta">—</span>
        )}
      </td>
    </tr>
  );
}

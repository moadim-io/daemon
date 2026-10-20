import { Link } from "react-router-dom";
import { useAllRuns, useMachine, useRoutines } from "../../api/hooks";
import { useNow } from "../../lib/useNow";
import { fmtRunDuration } from "../../lib/runDisplay";
import { RefreshFreshness, refreshMs, useRefreshToken } from "../../components/RefreshControl";
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
import { computeAdherence } from "./adherenceMath";
import { ScheduleAdherence } from "./ScheduleAdherence";
import { computeIncidents } from "./incidentMath";
import { FailureIncidents } from "./FailureIncidents";
import { computeFailureCauses } from "./exitCodeMath";
import { FailureCauses } from "./FailureCauses";
import { SortableTh } from "../../components/SortableTh";
import { sortRows, usePersistedSort, type SortAccessors } from "../../lib/tableSort";

/**
 * Fleet-wide runs fetched to build the reliability sample. Mirrors the Routines table's
 * sparkline fetch cap (`GET /routines/runs` truncates its newest-first merged list to this many
 * total, across every routine) — high enough that an active fleet's routines each keep a
 * `SAMPLE_LEN`-sized window without an unbounded payload.
 */
const FETCH_LIMIT = 300;

const REL_SORT_KEYS = ["routine", "streak", "rate", "p50", "p95", "trend"] as const;
type RelSortKey = (typeof REL_SORT_KEYS)[number];

/** Streak sorts failing-longest lowest, so ascending surfaces what's most broken first. */
const REL_SORT: SortAccessors<RoutineReliability, RelSortKey> = {
  routine: (i) => i.routineTitle,
  streak: (i) => (i.streak.kind === "failure" ? -i.streak.count : i.streak.kind === "success" ? i.streak.count : 0),
  rate: (i) => successRate(i),
  p50: (i) => i.p50Secs,
  p95: (i) => i.p95Secs,
  trend: (i) => (i.regressing ? 1 : 0),
};

function fmtSecs(secs: number | null): string {
  return secs === null ? "—" : fmtRunDuration(0, secs);
}

/**
 * The RELIABILITY page: ranks every routine by recent run outcomes and duration so an operator
 * can spot what's actively broken, flaky, or trending slower without opening each routine's
 * HISTORY tab individually.
 */
export function ReliabilityPage() {
  const refreshToken = useRefreshToken();
  const {
    data: runs,
    isLoading,
    error,
    dataUpdatedAt,
  } = useAllRuns(FETCH_LIMIT, { refetchInterval: refreshMs(refreshToken) });
  const { data: routines } = useRoutines({}, { refetchInterval: refreshMs(refreshToken) });
  const { data: machine, isLoading: machineLoading } = useMachine();
  const nowMs = useNow(60_000);

  const items = computeReliability(runs ?? []);
  const { sort, toggle: toggleSort } = usePersistedSort("moadim.client.reliability.sort", REL_SORT_KEYS);
  const summary = fleetSummary(items);
  const fleetRate = summary.sampleSize === 0 ? null : summary.successes / summary.sampleSize;
  const errorMessage = error?.message;
  const adherence = computeAdherence(routines ?? [], runs ?? [], machine?.name, Math.floor(nowMs / 1000), FETCH_LIMIT);
  const incidents = computeIncidents(runs ?? [], Math.floor(nowMs / 1000));
  const causes = computeFailureCauses(runs ?? []);

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Reliability</h1>
        <div className="section-acts">
          <RefreshFreshness updatedAtMs={dataUpdatedAt} />
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

      {/* Wait for `/machine`: until then every machine's routines would be audited as local. */}
      {!errorMessage && !isLoading && !machineLoading && <ScheduleAdherence adherence={adherence} />}
      {!errorMessage && !isLoading && <FailureIncidents report={incidents} />}
      {!errorMessage && !isLoading && <FailureCauses report={causes} />}

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
                <SortableTh label="ROUTINE" col="routine" sort={sort} defaultDir="asc" onSort={toggleSort} />
                <SortableTh label="STREAK" col="streak" sort={sort} defaultDir="asc" onSort={toggleSort} />
                <SortableTh label="SUCCESS RATE" col="rate" sort={sort} defaultDir="asc" onSort={toggleSort} />
                <SortableTh label="P50" col="p50" sort={sort} defaultDir="desc" onSort={toggleSort} />
                <SortableTh label="P95" col="p95" sort={sort} defaultDir="desc" onSort={toggleSort} />
                <SortableTh label="TREND" col="trend" sort={sort} defaultDir="desc" onSort={toggleSort} />
              </tr>
            </thead>
            <tbody>
              {sortRows(items, sort, REL_SORT).map((item) => (
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

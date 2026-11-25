import { useMemo } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { useAllRuns, useRoutines } from "../../api/hooks";
import { RefreshFreshness, refreshMs, useRefreshToken } from "../../components/RefreshControl";
import { fmtRunDuration } from "../../lib/runDisplay";
import { useNow } from "../../lib/useNow";
import { DEFAULT_RANGE, RANGE_OPTIONS, rangeFromParam } from "../uptime/uptimeMath";
import {
  buildUsage,
  changeClass,
  changeLabel,
  fmtAgentTime,
  groupByFromParam,
  usageDayLabel,
  type UsageDay,
  type UsageGroupBy,
  type UsageRow,
} from "./usageMath";

/** Fleet-wide runs fetched: covers two 90-day periods of a busy fleet; the page says when it falls short. */
const FETCH_LIMIT = 2000;

const GROUP_OPTIONS: UsageGroupBy[] = ["routine", "agent"];

const dayTotal = (d: UsageDay) => d.successSecs + d.failedSecs + d.otherSecs;

/**
 * The USAGE page: fleet agent-time insights — how many agent-hours the fleet burned per day, which
 * routine or agent consumed them, and the change versus the previous period of equal length.
 */
export function UsagePage() {
  const refreshToken = useRefreshToken();
  const ms = refreshMs(refreshToken);
  const routinesQuery = useRoutines({}, { refetchInterval: ms });
  const runsQuery = useAllRuns(FETCH_LIMIT, { refetchInterval: ms });
  const nowSecs = Math.floor(useNow(60_000) / 1000);
  const [params, setParams] = useSearchParams();
  const days = rangeFromParam(params.get("days"));
  const groupBy = groupByFromParam(params.get("by"));

  const report = useMemo(
    () => buildUsage(runsQuery.data ?? [], routinesQuery.data ?? [], nowSecs, days, groupBy, FETCH_LIMIT),
    [runsQuery.data, routinesQuery.data, nowSecs, days, groupBy],
  );
  const peak = Math.max(0, ...report.days.map(dayTotal));

  /** Sets `key` in the URL, or drops it when `value` is the default. */
  const setParam = (key: string, value: string, fallback: string) => {
    const next = new URLSearchParams(params);
    if (value === fallback) next.delete(key);
    else next.set(key, value);
    setParams(next, { replace: true });
  };

  const isLoading = routinesQuery.isLoading || runsQuery.isLoading;
  const errorMessage = routinesQuery.error?.message ?? runsQuery.error?.message;
  const updatedAtMs = Math.min(routinesQuery.dataUpdatedAt || Infinity, runsQuery.dataUpdatedAt || Infinity);

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Usage</h1>
        <div className="section-acts">
          <RefreshFreshness updatedAtMs={Number.isFinite(updatedAtMs) ? updatedAtMs : 0} />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">AGENT TIME</div>
          <div className="stat-val">{fmtAgentTime(report.totalSecs)}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">VS PREVIOUS {days}D</div>
          <div className={`stat-val ${changeClass(report.totalSecs, report.prevTotalSecs) ?? ""}`.trim()}>
            {changeLabel(report.totalSecs, report.prevTotalSecs)}
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-label">AVG / DAY</div>
          <div className="stat-val">{fmtAgentTime(report.totalSecs / days)}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">RUNS</div>
          <div className="stat-val">{report.runs}</div>
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
      ) : (
        <>
          <div className="filter-bar">
            <div className="uptime-range" role="group" aria-label="Range">
              {RANGE_OPTIONS.map((r) => (
                <button
                  key={r}
                  type="button"
                  className={r === days ? "btn btn-sm btn-primary" : "btn btn-sm"}
                  aria-pressed={r === days}
                  onClick={() => setParam("days", String(r), String(DEFAULT_RANGE))}
                >
                  {r}D
                </button>
              ))}
            </div>
            <div className="uptime-range" role="group" aria-label="Group by">
              {GROUP_OPTIONS.map((g) => (
                <button
                  key={g}
                  type="button"
                  className={g === groupBy ? "btn btn-sm btn-primary" : "btn btn-sm"}
                  aria-pressed={g === groupBy}
                  onClick={() => setParam("by", g, "routine")}
                >
                  BY {g.toUpperCase()}
                </button>
              ))}
            </div>
          </div>
          {report.partialSample ? (
            <div className="uptime-note cell-meta">
              Only the newest {FETCH_LIMIT} runs are loaded, so the oldest days and the previous period may be
              incomplete.
            </div>
          ) : null}
          {report.rows.length === 0 ? (
            <div className="table-wrap">
              <div className="empty">
                <div className="empty-icon">⏱</div>
                <div className="empty-msg">NO AGENT TIME YET</div>
                <div className="empty-sub">no run started in the last {days * 2} days</div>
              </div>
            </div>
          ) : (
            <>
              <div className="usage-chart" role="list" aria-label="Agent time per day">
                {report.days.map((day) => (
                  <DayBar key={day.from} day={day} peak={peak} />
                ))}
              </div>
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>{groupBy === "agent" ? "AGENT" : "ROUTINE"}</th>
                      <th>AGENT TIME</th>
                      <th>SHARE</th>
                      <th>RUNS</th>
                      <th>AVG RUN</th>
                      <th>PREV {days}D</th>
                      <th>CHANGE</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.rows.map((row) => (
                      <UsageTr key={row.key} row={row} groupBy={groupBy} total={report.totalSecs} />
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
}

function DayBar({ day, peak }: { day: UsageDay; peak: number }) {
  const label = usageDayLabel(day);
  const pct = (secs: number) => `${peak === 0 ? 0 : (secs / peak) * 100}%`;
  return (
    <div className="usage-day" role="listitem" title={label} aria-label={label}>
      <span className="usage-seg other" style={{ height: pct(day.otherSecs) }} />
      <span className="usage-seg failed" style={{ height: pct(day.failedSecs) }} />
      <span className="usage-seg success" style={{ height: pct(day.successSecs) }} />
    </div>
  );
}

function UsageTr({ row, groupBy, total }: { row: UsageRow; groupBy: UsageGroupBy; total: number }) {
  const href =
    groupBy === "agent"
      ? `/routines?agent=${encodeURIComponent(row.key)}`
      : `/routines/${encodeURIComponent(row.key)}`;
  const share = total === 0 ? 0 : row.secs / total;
  return (
    <tr>
      <td>
        <Link to={href}>{row.label}</Link>
      </td>
      <td>{fmtAgentTime(row.secs)}</td>
      <td>
        <span className="usage-share" aria-hidden="true">
          <span className="usage-share-bar" style={{ width: `${share * 100}%` }} />
        </span>{" "}
        <span className="cell-meta">{Math.round(share * 100)}%</span>
      </td>
      <td>{row.runs}</td>
      <td className="cell-meta">{row.runs === 0 ? "—" : fmtRunDuration(0, Math.round(row.secs / row.runs))}</td>
      <td className="cell-meta">{fmtAgentTime(row.prevSecs)}</td>
      <td className={changeClass(row.secs, row.prevSecs)}>{changeLabel(row.secs, row.prevSecs)}</td>
    </tr>
  );
}

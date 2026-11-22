import { useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { useAllRuns, useRoutines } from "../../api/hooks";
import { RefreshFreshness, refreshMs, useRefreshToken } from "../../components/RefreshControl";
import { useNow } from "../../lib/useNow";
import { rateClass } from "../reliability/reliabilityStats";
import { DEFAULT_RUNS_FILTER, viewToParams } from "../runs/runsFilter";
import {
  buildUptime,
  DEFAULT_RANGE,
  dayLabel,
  RANGE_OPTIONS,
  rangeFromParam,
  uptimeLabel,
  type DayCell,
  type DayState,
  type UptimeRange,
  type UptimeRow,
} from "./uptimeMath";

/** Fleet-wide runs fetched: enough to cover ~90 days of a busy fleet; the page says when the sample falls short. */
const FETCH_LIMIT = 2000;

const LEGEND: Record<DayState, string> = {
  ok: "all succeeded",
  partial: "some failed",
  down: "all failed",
  none: "no finished runs",
};

/**
 * The UPTIME page: a Statuspage-style daily status history — one row per routine, one bar per
 * calendar day — so an operator sees which days broke and whether it is a pattern, then clicks a
 * bar to open that day's runs.
 */
export function UptimePage() {
  const refreshToken = useRefreshToken();
  const ms = refreshMs(refreshToken);
  const routinesQuery = useRoutines({}, { refetchInterval: ms });
  const runsQuery = useAllRuns(FETCH_LIMIT, { refetchInterval: ms });
  const nowSecs = Math.floor(useNow(60_000) / 1000);
  const [params, setParams] = useSearchParams();
  const days = rangeFromParam(params.get("days"));
  const [query, setQuery] = useState("");

  const report = useMemo(
    () => buildUptime(runsQuery.data ?? [], routinesQuery.data ?? [], nowSecs, days, FETCH_LIMIT),
    [runsQuery.data, routinesQuery.data, nowSecs, days],
  );
  const q = query.trim().toLowerCase();
  const shown = q === "" ? report.rows : report.rows.filter((r) => r.title.toLowerCase().includes(q));

  const setDays = (range: UptimeRange) => {
    const next = new URLSearchParams(params);
    if (range === DEFAULT_RANGE) next.delete("days");
    else next.set("days", String(range));
    setParams(next, { replace: true });
  };

  const isLoading = routinesQuery.isLoading || runsQuery.isLoading;
  const errorMessage = routinesQuery.error?.message ?? runsQuery.error?.message;
  const updatedAtMs = Math.min(routinesQuery.dataUpdatedAt || Infinity, runsQuery.dataUpdatedAt || Infinity);

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Uptime</h1>
        <div className="section-acts">
          <RefreshFreshness updatedAtMs={Number.isFinite(updatedAtMs) ? updatedAtMs : 0} />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">FLEET UPTIME</div>
          <div className="stat-val">{uptimeLabel(report.fleet.uptime)}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">DAYS WITH FAILURES</div>
          <div className={report.daysWithFailures > 0 ? "stat-val c-red" : "stat-val"}>
            {report.daysWithFailures} / {days}
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-label">ROUTINES AT 100%</div>
          <div className="stat-val">{report.perfect}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">FINISHED RUNS</div>
          <div className="stat-val">{report.fleet.success + report.fleet.failed}</div>
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
      ) : report.rows.length === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">▮</div>
            <div className="empty-msg">NO ROUTINES YET</div>
            <div className="empty-sub">create a routine to start its daily status history</div>
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
                  onClick={() => setDays(r)}
                >
                  {r}D
                </button>
              ))}
            </div>
            <input
              type="search"
              className="search-input"
              placeholder="Search routines…"
              aria-label="Search routines"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <div className="uptime-legend" aria-hidden="true">
              {(Object.keys(LEGEND) as DayState[]).map((s) => (
                <span key={s} className="uptime-legend-item">
                  <span className={`uptime-cell ${s}`} /> {LEGEND[s]}
                </span>
              ))}
            </div>
          </div>
          {report.partialSample ? (
            <div className="uptime-note cell-meta">
              Only the newest {FETCH_LIMIT} runs are loaded, so the oldest days may be incomplete.
            </div>
          ) : null}
          <div className="table-wrap">
            <table className="uptime-table">
              <thead>
                <tr>
                  <th>ROUTINE</th>
                  <th>LAST {days} DAYS</th>
                  <th>UPTIME</th>
                </tr>
              </thead>
              <tbody>
                <UptimeTr row={report.fleet} fleet />
                {shown.map((row) => (
                  <UptimeTr key={row.routineId} row={row} />
                ))}
              </tbody>
            </table>
            {shown.length === 0 ? (
              <div className="empty">
                <div className="empty-msg">NO MATCHING ROUTINES</div>
              </div>
            ) : null}
          </div>
        </>
      )}
    </div>
  );
}

function UptimeTr({ row, fleet = false }: { row: UptimeRow; fleet?: boolean }) {
  return (
    <tr className={fleet ? "uptime-fleet" : undefined}>
      <td>
        {fleet ? (
          <strong>All routines</strong>
        ) : (
          <Link to={`/routines/${encodeURIComponent(row.routineId)}`}>{row.title}</Link>
        )}
      </td>
      <td>
        <div className="uptime-strip">
          {row.days.map((day) => (
            <DayBar key={day.from} day={day} query={fleet ? "" : row.title} />
          ))}
        </div>
      </td>
      <td>
        <span className={rateClass(row.uptime)}>{uptimeLabel(row.uptime)}</span>
      </td>
    </tr>
  );
}

function DayBar({ day, query }: { day: DayCell; query: string }) {
  const label = dayLabel(day);
  const className = `uptime-cell ${day.state}${day.running ? " live" : ""}`;
  if (day.state === "none" && !day.running) {
    return <span className={className} role="img" title={label} aria-label={label} />;
  }
  // ponytail: the Runs page matches `q` as a title substring, so a routine whose title contains
  // another's also shows that one's runs; an exact routine-id filter on /runs would fix it.
  const href = `/runs?${viewToParams({ ...DEFAULT_RUNS_FILTER, query }, { from: day.from, to: day.to }).toString()}`;
  return <Link className={className} to={href} title={label} aria-label={label} />;
}

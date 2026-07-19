import { Fragment, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useRoutines } from "../../api/hooks";
import {
  loadRefreshToken,
  refreshMs,
  RefreshControl,
  saveRefreshToken,
  type RefreshToken,
} from "../../components/RefreshControl";
import { dateOnly } from "../../lib/schedule";
import { DayTimeline } from "./DayTimeline";
import { timelineItemsOf } from "./dayTimelineMath";
import {
  computeHeatmap,
  dayLabel,
  dayTotals,
  filledCells,
  heatFilterLabel,
  HEAT_DAYS,
  HEAT_HOURS,
  hourTotals,
  intensityLevel,
  peakLabel,
  sourcesOf,
  type HeatFilter,
} from "./heatmapMath";

/** How often "now" advances so the grid (and its today/current-hour highlight)
 * rolls forward between fetches. Mirrors `ui/src/schedule_heatmap.rs`'s `TICK_MS`. */
const TICK_MS = 60_000;

const HEAT_FILTERS: HeatFilter[] = ["all", "routine"];

function addDays(base: Date, n: number): Date {
  return new Date(base.getFullYear(), base.getMonth(), base.getDate() + n);
}

function fmtDayParam(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function parseDayParam(s: string | null): Date | undefined {
  const match = s ? /^(\d{4})-(\d{2})-(\d{2})$/.exec(s) : null;
  if (!match) return undefined;
  const [, y, m, d] = match;
  return new Date(Number(y), Number(m) - 1, Number(d));
}

/** Cell background intensity as a percent mix of `--accent` over `--surface`,
 * driven by the ported `intensityLevel` bucket (0 = empty, 1-4 = quartile ramp)
 * so cell color always themes correctly in light/dark instead of hardcoding hex. */
function cellBackground(level: number): string | undefined {
  if (level === 0) return undefined;
  return `color-mix(in srgb, var(--accent) ${10 + level * 20}%, var(--surface))`;
}

export function HeatmapPage() {
  const [refreshToken, setRefreshToken] = useState<RefreshToken>(loadRefreshToken);
  const {
    data: routines,
    isLoading,
    error,
    dataUpdatedAt,
  } = useRoutines({}, { refetchInterval: refreshMs(refreshToken) });
  const [now, setNow] = useState(() => new Date());
  const [filter, setFilter] = useState<HeatFilter>("all");
  const [searchParams, setSearchParams] = useSearchParams();

  const onChangeRefresh = (next: RefreshToken) => {
    saveRefreshToken(next);
    setRefreshToken(next);
  };

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), TICK_MS);
    return () => clearInterval(id);
  }, []);

  const today = dateOnly(now);
  const currentHour = now.getHours();
  const sources = useMemo(() => sourcesOf(routines ?? []), [routines]);
  const map = useMemo(() => computeHeatmap(sources, now, filter), [sources, now, filter]);

  const selectedDay = parseDayParam(searchParams.get("day"));
  const openDay = (day: Date) => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev);
      next.set("day", fmtDayParam(day));
      return next;
    });
  };
  const closeDay = () => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev);
      next.delete("day");
      return next;
    });
  };

  const timelineItems = useMemo(() => timelineItemsOf(routines ?? [], now), [routines, now]);

  const errorMessage = error?.message;
  const dayOffsets = Array.from({ length: HEAT_DAYS }, (_, i) => i);
  const hours = Array.from({ length: HEAT_HOURS }, (_, i) => i);
  const daySums = dayTotals(map);
  const hourSums = hourTotals(map);
  const openSlots = HEAT_DAYS * HEAT_HOURS - filledCells(map);

  return (
    <div className="page">
      <div className="hm-hd">
        <h1 className="page-title">Heatmap</h1>
        <div className="hm-hd-acts">
          <div className="hm-filter" role="group" aria-label="Source filter">
            {HEAT_FILTERS.map((f) => (
              <button
                key={f}
                className={f === filter ? "hm-filter-btn active" : "hm-filter-btn"}
                aria-pressed={f === filter}
                onClick={() => setFilter(f)}
              >
                {heatFilterLabel(f)}
              </button>
            ))}
          </div>
          <RefreshControl token={refreshToken} updatedAtMs={dataUpdatedAt} onChange={onChangeRefresh} />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">FIRES / 7 DAYS</div>
          <div className="stat-val">{map.total}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">BUSIEST WINDOW</div>
          <div className="stat-val stat-val-sm c-accent">{peakLabel(map, today) ?? "—"}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">PEAK / HOUR</div>
          <div className="stat-val">{map.maxCell}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">SOURCES</div>
          <div className="stat-val">{map.sources}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">OPEN SLOTS</div>
          <div className="stat-val stat-val-sm">
            {openSlots} / {HEAT_DAYS * HEAT_HOURS}
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
      ) : map.total === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">▦</div>
            <div className="empty-msg">NOTHING SCHEDULED</div>
            <div className="empty-sub">no enabled routine fires in the next 7 days</div>
          </div>
        </div>
      ) : (
        <>
          <div className="hm-grid-wrap">
            <div className="hm-grid" role="grid" aria-label="Fire density by day and hour">
              <div className="hm-corner" role="columnheader">
                DAY \ HR
              </div>
              {hours.map((hour) => (
                <div
                  key={hour}
                  className={hour === currentHour ? "hm-hcol now" : "hm-hcol"}
                  role="columnheader"
                >
                  {String(hour).padStart(2, "0")}
                </div>
              ))}
              <div className="hm-rowtot hm-corner" role="columnheader">
                Σ
              </div>

              {dayOffsets.map((day) => {
                const date = addDays(today, day);
                return (
                  <Fragment key={day}>
                    <button
                      className={day === 0 ? "hm-daylabel today" : "hm-daylabel"}
                      onClick={() => openDay(date)}
                      title={`View ${dayLabel(today, day)}`}
                    >
                      {dayLabel(today, day)}
                    </button>
                    {hours.map((hour) => {
                      const count = map.grid[day]?.[hour] ?? 0;
                      const level = intensityLevel(count, map.maxCell);
                      const isPeak = map.peak?.[0] === day && map.peak[1] === hour;
                      const isNow = day === 0 && hour === currentHour;
                      let cls = "hm-cell";
                      if (isPeak) cls += " peak";
                      if (isNow) cls += " now";
                      return (
                        <button
                          key={`${day}-${hour}`}
                          className={cls}
                          style={{ backgroundColor: cellBackground(level) }}
                          onClick={() => openDay(date)}
                          title={`${dayLabel(today, day)} ${String(hour).padStart(2, "0")}:00 — ${count} run${count === 1 ? "" : "s"}`}
                        >
                          {count > 0 ? <span className="hm-n">{count}</span> : null}
                        </button>
                      );
                    })}
                    <div className="hm-rowtot">{daySums[day]}</div>
                  </Fragment>
                );
              })}

              <div className="hm-daylabel hm-foot">Σ</div>
              {hours.map((hour) => (
                <div key={`hourtot-${hour}`} className="hm-rowtot hm-foot">
                  {hourSums[hour]}
                </div>
              ))}
              <div className="hm-rowtot hm-foot grand">{map.total}</div>
            </div>
          </div>

          <div className="hm-legend">
            <span className="hm-legend-label">LESS</span>
            {[0, 1, 2, 3, 4].map((level) => (
              <span
                key={level}
                className="hm-cell hm-legend-swatch"
                style={{ backgroundColor: cellBackground(level) }}
              />
            ))}
            <span className="hm-legend-label">MORE</span>
            <span className="hm-legend-note">
              fires per weekday · hour across the next 7 days
            </span>
          </div>
        </>
      )}

      {selectedDay && (
        <DayTimeline
          day={selectedDay}
          items={timelineItems}
          loading={isLoading}
          isToday={dateOnly(selectedDay).getTime() === today.getTime()}
          currentHour={currentHour}
          onPrev={() => openDay(addDays(selectedDay, -1))}
          onNext={() => openDay(addDays(selectedDay, 1))}
          onToday={() => openDay(today)}
          onClose={closeDay}
        />
      )}
    </div>
  );
}

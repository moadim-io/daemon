import { abstime } from "../../lib/cronUtils";
import { fmtRunDuration } from "../../lib/runDisplay";
import { pct } from "../runs/ganttMath";
import type { Forecast } from "./forecastMath";

export interface CapacityForecastProps {
  forecast: Forecast;
  /** Effective `max_concurrent_runs` cap; `0` = unbounded, `undefined` while loading. */
  cap: number | undefined;
}

function hm(ts: number): string {
  return abstime(ts).slice(-5);
}

/**
 * Next-24h capacity forecast: each enabled routine's upcoming fires drawn as bars sized by its
 * median historical run duration, a projected concurrency strip with the cap line, and the
 * windows where projected runs exceed the cap (and would queue) — the forward-looking
 * counterpart to the Runs page's Gantt.
 */
export function CapacityForecast({ forecast, cap }: CapacityForecastProps) {
  const { from, to, lanes, concurrency, peak, overCap } = forecast;
  if (lanes.length === 0) return null;
  const capped = cap !== undefined && cap > 0;
  const capLabel = cap === undefined ? "" : cap === 0 ? " (no cap)" : ` / cap ${cap}`;
  return (
    <details className="runs-gantt cap-forecast" open>
      <summary className="runs-gantt-hd">
        <span className="filter-label">CAPACITY FORECAST · NEXT 24H</span>
        <span className={overCap.length > 0 ? "runs-gantt-peak c-amber" : "runs-gantt-peak"}>
          projected peak {peak}
          {capLabel}
          {overCap.length > 0 && ` — ${overCap.length} over-cap window${overCap.length === 1 ? "" : "s"}`}
        </span>
      </summary>
      <div className="runs-gantt-row">
        <span className="runs-gantt-name">projected</span>
        <div
          className="runs-gantt-track runs-gantt-strip"
          role="img"
          aria-label={`Projected concurrent runs over the next 24 hours, peak ${peak}`}
        >
          {concurrency
            .filter((s) => s.count > 0)
            .map((s) => (
              <span
                key={s.from}
                className={capped && s.count > cap ? "runs-gantt-level over" : "runs-gantt-level"}
                title={`${abstime(s.from)} – ${abstime(s.to)}: ${s.count} projected`}
                style={{
                  left: `${pct(s.from, from, to)}%`,
                  width: `${pct(s.to, from, to) - pct(s.from, from, to)}%`,
                  height: `${(s.count / peak) * 100}%`,
                }}
              />
            ))}
          {capped && cap <= peak && <span className="runs-gantt-cap" style={{ bottom: `${(cap / peak) * 100}%` }} />}
        </div>
      </div>
      <div className="runs-gantt-lanes">
        {lanes.map((lane) => {
          const est = `${fmtRunDuration(0, lane.estimateSecs)}${lane.estimated ? " (default, no history)" : " median"}`;
          return (
            <div className="runs-gantt-row" key={lane.routineId}>
              <span className="runs-gantt-name" title={`${lane.title} · ~${est}`}>
                {lane.title}
              </span>
              <div className="runs-gantt-track">
                {lane.bars.map((b) => {
                  const label = `${lane.title}: ${b.running ? "running now" : `fires ${abstime(b.start)}`}, ~${est}`;
                  return (
                    <span
                      key={b.start}
                      className={b.running ? "runs-gantt-bar running" : lane.estimated ? "runs-gantt-bar est" : "runs-gantt-bar"}
                      role="img"
                      aria-label={label}
                      title={label}
                      style={{ left: `${pct(b.start, from, to)}%`, width: `${pct(b.end, from, to) - pct(b.start, from, to)}%` }}
                    />
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
      <div className="runs-hist-axis runs-gantt-axis">
        <span>now</span>
        <span>+24h</span>
      </div>
      {overCap.length > 0 && (
        <ul className="cap-forecast-windows" aria-label="Over-cap windows">
          {overCap.map((w) => (
            <li key={w.from}>
              <span className="c-amber">
                {hm(w.from)}–{hm(w.to)} · {w.peak} projected
              </span>{" "}
              {w.titles.join(", ")}
            </li>
          ))}
        </ul>
      )}
    </details>
  );
}

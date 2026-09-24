import { Link } from "react-router-dom";
import { abstime } from "../../lib/cronUtils";
import { fmtRunDuration } from "../../lib/runDisplay";
import { pct, type Gantt, type GanttBar } from "./ganttMath";

export interface RunsGanttProps {
  gantt: Gantt;
  /** Effective `max_concurrent_runs` cap; `0` = unbounded, `undefined` while loading. */
  cap: number | undefined;
}

function barLabel(title: string, b: GanttBar): string {
  const dur = b.run.status === "running" ? "running" : fmtRunDuration(b.run.started_at, b.end);
  return `${title}: started ${abstime(b.run.started_at)}, ${dur}, ${b.run.status}`;
}

/**
 * Gantt-style run timeline: one swimlane per routine, each run a bar from start to finish
 * (running runs extend to now), with a fleet concurrency strip on top and the peak compared to
 * the daemon's concurrency cap — the duration/overlap view workflow schedulers (Airflow's Gantt,
 * CI waterfalls) ship for spotting contention.
 */
export function RunsGantt({ gantt, cap }: RunsGanttProps) {
  const { from, to, lanes, concurrency, peak } = gantt;
  if (lanes.length === 0) return null;
  const capped = cap !== undefined && cap > 0;
  const atCap = capped && peak >= cap;
  const capLabel = cap === undefined ? "" : cap === 0 ? " (no cap)" : ` / cap ${cap}`;
  return (
    <details className="runs-gantt" open>
      <summary className="runs-gantt-hd">
        <span className="filter-label">TIMELINE</span>
        <span className={atCap ? "runs-gantt-peak c-amber" : "runs-gantt-peak"}>
          peak concurrency {peak}
          {capLabel}
          {atCap && " — runs may have queued"}
        </span>
      </summary>
      <div className="runs-gantt-row">
        <span className="runs-gantt-name">concurrent</span>
        <div className="runs-gantt-track runs-gantt-strip" role="img" aria-label={`Concurrent runs over time, peak ${peak}`}>
          {concurrency
            .filter((s) => s.count > 0)
            .map((s) => (
              <span
                key={s.from}
                className="runs-gantt-level"
                title={`${abstime(s.from)} – ${abstime(s.to)}: ${s.count} running`}
                style={{
                  left: `${pct(s.from, from, to)}%`,
                  width: `${pct(s.to, from, to) - pct(s.from, from, to)}%`,
                  height: `${(s.count / peak) * 100}%`,
                }}
              />
            ))}
          {capped && cap <= peak && (
            <span className="runs-gantt-cap" style={{ bottom: `${(cap / peak) * 100}%` }} />
          )}
        </div>
      </div>
      <div className="runs-gantt-lanes">
        {lanes.map((lane) => (
          <div className="runs-gantt-row" key={lane.routineId}>
            <span className="runs-gantt-name" title={lane.title}>
              {lane.title}
            </span>
            <div className="runs-gantt-track">
              {lane.bars.map((b) => {
                const label = barLabel(lane.title, b);
                return (
                  <Link
                    key={b.run.workbench}
                    to={`/runs/${encodeURIComponent(b.run.routine_id)}/${encodeURIComponent(b.run.workbench)}`}
                    className={`runs-gantt-bar ${b.run.status}`}
                    aria-label={label}
                    title={label}
                    style={{ left: `${pct(b.start, from, to)}%`, width: `${pct(b.end, from, to) - pct(b.start, from, to)}%` }}
                  />
                );
              })}
            </div>
          </div>
        ))}
      </div>
      <div className="runs-hist-axis runs-gantt-axis">
        <span>{abstime(from)}</span>
        <span>now</span>
      </div>
    </details>
  );
}

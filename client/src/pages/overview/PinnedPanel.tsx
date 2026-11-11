import { Link } from "react-router-dom";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { usePins } from "../../lib/pins";
import { healthBadge, healthBadgeClass, healthTooltip, routineHealth } from "../routines/filter";
import { NextRunCell } from "../routines/RoutineRow";
import { RunHistorySparkline } from "../routines/RunHistorySparkline";

export interface PinnedPanelProps {
  routines: RoutineResponse[];
  /** Each routine's recent runs, oldest first (as produced by `groupRecentRuns`). */
  runHistory: Map<string, FleetRunSummary[]>;
  now: Date;
  onTrigger: (id: string) => void;
}

/**
 * The Overview page's PINNED watchlist: the operator's hand-picked routines with health,
 * next run, recent-run trend and a one-click RUN. Pins for deleted routines are skipped.
 */
export function PinnedPanel({ routines, runHistory, now, onTrigger }: PinnedPanelProps) {
  const { pins, toggle } = usePins();
  const byId = new Map(routines.map((r) => [r.id, r]));
  const pinned = pins.flatMap((id) => {
    const r = byId.get(id);
    return r === undefined ? [] : [r];
  });

  return (
    <>
      <div className="section-hd">
        <span className="section-label">PINNED</span>
      </div>
      {pinned.length === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">☆</div>
            <div className="empty-msg">NO PINNED ROUTINES</div>
            <div className="empty-sub">
              Pin routines with ☆ on the <Link to="/routines">Routines</Link> page to watch them here
            </div>
          </div>
        </div>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>NAME</th>
                <th>HEALTH</th>
                <th>NEXT RUN</th>
                <th>LAST RUNS</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {pinned.map((r) => {
                const health = routineHealth(r, now);
                return (
                  <tr key={r.id}>
                    <td>
                      <Link className="ov-name-link" to={`/routines?history=${encodeURIComponent(r.id)}`}>
                        {r.title}
                      </Link>
                    </td>
                    <td>
                      <span className={healthBadgeClass(health)} title={healthTooltip(r, health)}>
                        {healthBadge(health)}
                      </span>
                    </td>
                    <td className="cell-next">
                      <NextRunCell routine={r} now={now} />
                    </td>
                    <td>
                      <RunHistorySparkline runs={runHistory.get(r.id) ?? []} />
                    </td>
                    <td className="cell-act">
                      <button
                        type="button"
                        className="btn btn-sm btn-ghost run-now-btn"
                        title="Trigger now"
                        aria-label={`Run ${r.title} now`}
                        disabled={!r.enabled || r.power_saving}
                        onClick={() => onTrigger(r.id)}
                      >
                        ▶ RUN
                      </button>
                      <button
                        type="button"
                        className="pin-btn pinned"
                        title="Unpin"
                        aria-label={`Unpin ${r.title}`}
                        onClick={() => toggle(r.id)}
                      >
                        ★
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

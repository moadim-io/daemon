import type { RoutineResponse } from "../../api/hooks";
import { firesWithin } from "../../lib/schedule";
import { DUE_SOON_WINDOW_MS, isRoutineSnoozed, type RoutineStatusFacet } from "./filter";

export interface StatsBarProps {
  routines: RoutineResponse[];
  now: Date;
  active: RoutineStatusFacet;
  onStatus: (s: RoutineStatusFacet) => void;
}

/** KPI tiles above the routine list; clicking one applies (or clears) a status facet. */
export function StatsBar({ routines, now, active, onStatus }: StatsBarProps) {
  const total = routines.length;
  const enabled = routines.filter((r) => r.enabled).length;
  const disabled = total - enabled;
  const dueSoon = routines.filter(
    (r) => r.enabled && !isRoutineSnoozed(r, now) && firesWithin(r.schedule, now, DUE_SOON_WINDOW_MS),
  ).length;
  const snoozed = routines.filter((r) => r.enabled && isRoutineSnoozed(r, now)).length;
  const dormant = routines.filter((r) => r.enabled && (r.machines ?? []).length === 0).length;
  const flags = routines.reduce((sum, r) => sum + (r.flag_count ?? 0), 0);
  const unreg = routines.filter((r) => !r.agent_registered).length;

  const tiles: [RoutineStatusFacet, string, number, string][] = [
    ["all", "TOTAL", total, ""],
    ["enabled", "ENABLED", enabled, ""],
    ["disabled", "DISABLED", disabled, ""],
    ["dormant", "DORMANT", dormant, dormant > 0 ? "has-dormant" : ""],
    ["due", "DUE SOON", dueSoon, ""],
    ["snoozed", "SNOOZED", snoozed, ""],
    ["flagged", "FLAGS", flags, flags > 0 ? "has-flags" : ""],
    ["agent-unreg", "UNREGISTERED AGENT", unreg, unreg > 0 ? "has-unreg" : ""],
  ];

  return (
    <div className="stats">
      {tiles.map(([facet, label, val, extraCls]) => {
        const isActive = active === facet;
        return (
          <button
            key={facet}
            type="button"
            className={`stat-card ${extraCls}${isActive ? " active" : ""}`.trim()}
            aria-pressed={isActive}
            onClick={() => onStatus(isActive ? "all" : facet)}
          >
            <div className="stat-label">{label}</div>
            <div className="stat-val">{val}</div>
          </button>
        );
      })}
    </div>
  );
}

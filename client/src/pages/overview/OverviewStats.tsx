import type { Kpis } from "./overviewLogic";

/** The Overview page's KPI tile row. Direct port of `ui/src/overview_stats.rs (removed)`. */
export function OverviewStats({ kpis: k, nextRun }: { kpis: Kpis; nextRun: string | undefined }) {
  return (
    <div className="stats">
      <div className="stat-card">
        <div className="stat-label">SCHEDULED</div>
        <div className="stat-val">{k.total}</div>
      </div>
      <div className="stat-card">
        <div className="stat-label">ENABLED</div>
        <div className="stat-val c-accent">{k.enabled}</div>
      </div>
      <div className="stat-card">
        <div className="stat-label">DUE SOON</div>
        <div className="stat-val c-red">{k.dueSoon}</div>
      </div>
      <div className="stat-card">
        <div className="stat-label">ATTENTION</div>
        <div className={`stat-val ${k.attention > 0 ? "c-red" : "c-accent"}`}>{k.attention}</div>
      </div>
      <div className="stat-card">
        <div className="stat-label">DISABLED</div>
        <div className="stat-val c-amber">{k.disabled}</div>
      </div>
      <div className="stat-card">
        <div className="stat-label">DORMANT</div>
        <div className={`stat-val ${k.dormant > 0 ? "c-amber" : ""}`}>{k.dormant}</div>
      </div>
      <div className="stat-card">
        <div className="stat-label">FLAGS</div>
        <div className={`stat-val ${k.flags > 0 ? "c-red" : "c-accent"}`}>{k.flags}</div>
      </div>
      <div className="stat-card">
        <div className="stat-label">SNOOZED</div>
        <div className={`stat-val ${k.snoozed > 0 ? "c-amber" : "c-accent"}`}>{k.snoozed}</div>
      </div>
      <div className="stat-card stat-card-wide">
        <div className="stat-label">NEXT RUN</div>
        <div className="stat-val stat-val-sm">{nextRun ?? "—"}</div>
      </div>
    </div>
  );
}

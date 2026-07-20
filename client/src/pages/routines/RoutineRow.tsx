import { useState } from "react";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { abstime, reltime } from "../../lib/cronUtils";
import { fmtUntil, fmtWhen, nextFireAfter, nextFires } from "../../lib/schedule";
import {
  DUE_SOON_WINDOW_MS,
  healthBadge,
  healthBadgeClass,
  isRoutineSnoozed,
  lastFireAt,
  routineHealth,
  snoozeDetail,
  triggerButtonTitle,
} from "./filter";
import { RunHistorySparkline } from "./RunHistorySparkline";
import { formatTtl } from "./ttl";

/** The NEXT RUN cell, shared by the table (no row) — kept as a function for row.rs parity. */
export function NextRunCell({ routine, now }: { routine: RoutineResponse; now: Date }) {
  if (!routine.enabled) {
    return <span className="cell-next muted">paused</span>;
  }
  if (isRoutineSnoozed(routine, now)) {
    const detail = snoozeDetail(routine, now);
    return (
      <>
        <span className="cell-next muted">snoozed</span>
        {detail !== "" && <div className="cell-next-until muted">{detail}</div>}
      </>
    );
  }
  const then = nextFireAfter(routine.schedule, now);
  if (then === undefined) {
    return <span className="cell-next muted">—</span>;
  }
  const soon = then.getTime() - now.getTime() <= DUE_SOON_WINDOW_MS;
  return (
    <>
      <div className="cell-next-when">{fmtWhen(now, then)}</div>
      <div className={soon ? "cell-next-until soon" : "cell-next-until"}>{fmtUntil(now, then)}</div>
    </>
  );
}

export interface RoutineRowProps {
  routine: RoutineResponse;
  now: Date;
  runs: FleetRunSummary[];
  selected: boolean;
  onSelect: (id: string) => void;
  onEdit: (id: string) => void;
  onClone: (id: string) => void;
  onDelete: (id: string, title: string) => void;
  onToggle: (id: string, enabled: boolean) => void;
  onTrigger: (id: string) => void;
  onLogs: (id: string) => void;
  onHistory: (id: string) => void;
  onFlags: (id: string) => void;
}

export function RoutineRow({
  routine: r,
  now,
  runs,
  selected,
  onSelect,
  onEdit,
  onClone,
  onDelete,
  onToggle,
  onTrigger,
  onLogs,
  onHistory,
  onFlags,
}: RoutineRowProps) {
  const [previewOpen, setPreviewOpen] = useState(false);

  const cronText = r.schedule_description ?? "—";
  const updated = reltime(r.updated_at);
  const repos = r.repositories ?? [];
  const machines = (r.machines ?? []).filter((m) => m.trim() !== "");
  const tags = r.tags ?? [];
  const goalFirstLine = r.goal?.trim() ? r.goal.split("\n")[0] : undefined;

  const lastFire = lastFireAt(r);
  const manualIsLatest =
    r.last_manual_trigger_at != null &&
    (r.last_scheduled_trigger_at == null || r.last_manual_trigger_at >= r.last_scheduled_trigger_at);

  const health = routineHealth(r, now);

  return (
    <tr className={selected ? "row-selected" : ""}>
      <td className="col-select">
        <input
          type="checkbox"
          checked={selected}
          onChange={() => onSelect(r.id)}
          aria-label={`Select ${r.title}`}
        />
      </td>
      <td>
        <div className="cell-schedule" title={r.title}>
          <RoutineTitle title={r.title} />
        </div>
        {goalFirstLine !== undefined && (
          <div className="cell-goal" title={r.goal ?? ""}>
            {goalFirstLine}
          </div>
        )}
      </td>
      <td>
        <div className="cell-schedule">{r.schedule}</div>
        <div className="cell-schedule-human">{cronText}</div>
        <button
          type="button"
          className={previewOpen ? "sched-preview-btn open" : "sched-preview-btn"}
          aria-expanded={previewOpen}
          onClick={(e) => {
            e.stopPropagation();
            setPreviewOpen((v) => !v);
          }}
        >
          ▸ fires
        </button>
        {previewOpen && <FiresPanel schedule={r.schedule} now={now} />}
      </td>
      <td>
        <NextRunCell routine={r} now={now} />
      </td>
      <td>
        {lastFire === undefined ? (
          <span className="muted">—</span>
        ) : (
          <div className="cell-triggered">
            {manualIsLatest ? "↻" : "⏱"} {reltime(lastFire)}
          </div>
        )}
      </td>
      <td>
        <RunHistorySparkline runs={runs} />
      </td>
      <td>
        <span
          className="cell-handler"
          title={r.agent_registered ? "agent registered" : "agent config missing"}
        >
          <span className={r.agent_registered ? "handler-dot ok" : "handler-dot warn"} />
          {r.agent}
        </span>
        {r.model && <div className="cell-goal">{r.model}</div>}
      </td>
      <td>
        {repos.length === 0 ? (
          "—"
        ) : (
          <span title={repos.map((x) => x.repository).join("\n")}>{repos.length}</span>
        )}
      </td>
      <td className={machines.length === 0 ? "cell-meta cell-no-machines" : "cell-meta"}>
        {machines.length === 0 ? "—" : <span title={machines.join("\n")}>{machines.length}</span>}
      </td>
      <td>{tags.length === 0 ? "—" : <span title={tags.join(", ")}>{tags.join(", ")}</span>}</td>
      <td>
        <span className="cell-meta" title="workbench retention for finished runs">
          {formatTtl(r.ttl_secs)}
        </span>
      </td>
      <td>
        <span className={healthBadgeClass(health)} title={healthBadge(health)}>
          {healthBadge(health)}
        </span>
      </td>
      <td>
        <label className="toggle">
          <input type="checkbox" checked={r.enabled} onChange={(e) => onToggle(r.id, e.target.checked)} />
          <div className="toggle-track" />
        </label>
      </td>
      <td>
        <div className="cell-time" title={abstime(r.updated_at)}>
          {updated}
        </div>
      </td>
      <td>
        <div className="row-actions">
          <button
            type="button"
            className="act-btn run"
            title={triggerButtonTitle(r)}
            aria-label="Run now"
            disabled={!r.enabled || r.power_saving}
            onClick={() => onTrigger(r.id)}
          >
            ▶
          </button>
          <button type="button" className="act-btn logs" onClick={() => onLogs(r.id)}>
            LOGS
          </button>
          <button type="button" className="act-btn history" title="Run history" onClick={() => onHistory(r.id)}>
            HISTORY
          </button>
          <button type="button" className="act-btn flags" title="Open flags" onClick={() => onFlags(r.id)}>
            FLAGS
            {(r.flag_count ?? 0) > 0 && <span className="flag-badge">{r.flag_count}</span>}
          </button>
          <button type="button" className="act-btn edit" onClick={() => onEdit(r.id)}>
            EDIT
          </button>
          <button
            type="button"
            className="act-btn clone"
            title="Duplicate routine"
            aria-label="Duplicate routine"
            onClick={() => onClone(r.id)}
          >
            ⧉
          </button>
          <button
            type="button"
            className="act-btn del"
            title="Delete routine"
            aria-label="Delete routine"
            onClick={() => onDelete(r.id, r.title)}
          >
            ✕
          </button>
        </div>
      </td>
    </tr>
  );
}

function RoutineTitle({ title }: { title: string }) {
  const parts = title.split("/").filter(Boolean);
  if (parts.length <= 1) return <>{title}</>;
  return (
    <span className="cell-title-path">
      {parts.map((part, idx) => (
        <span key={`${part}-${idx}`}>
          {idx > 0 && <span className="cell-title-sep">/</span>}
          <span className={idx === parts.length - 1 ? "cell-title-leaf" : "cell-title-folder"}>
            {part}
          </span>
        </span>
      ))}
    </span>
  );
}

function FiresPanel({ schedule, now }: { schedule: string; now: Date }) {
  const fires = nextFires(schedule, now, 10);
  if (fires.length === 0) {
    return (
      <div className="fires-panel">
        <div className="fires-empty">— no future fires —</div>
      </div>
    );
  }
  return (
    <div className="fires-panel">
      <div className="fires-hd">NEXT 10 FIRES</div>
      {fires.map((then, i) => (
        <div className="fires-item" key={i}>
          <span className="fires-when">{fmtWhen(now, then)}</span>
          <span className="fires-until">{fmtUntil(now, then)}</span>
        </div>
      ))}
    </div>
  );
}

import { useState } from "react";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { abstime, reltime } from "../../lib/cronUtils";
import { fmtUntil, fmtWhen, nextFireAfterAny, nextFiresAny, scheduleList } from "../../lib/schedule";
import {
  DUE_SOON_WINDOW_MS,
  failureRisk,
  healthBadge,
  healthBadgeClass,
  healthTooltip,
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
  const then = nextFireAfterAny(scheduleList(routine), now);
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
  onMove: (id: string) => void;
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
  onMove,
  onToggle,
  onTrigger,
  onLogs,
  onHistory,
  onFlags,
}: RoutineRowProps) {
  const [previewOpen, setPreviewOpen] = useState(false);

  const schedules = scheduleList(r);
  const cronText = (r.schedule_descriptions ?? [r.schedule_description]).filter(Boolean).join(" · ") || "—";
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
  const risk = failureRisk(r);

  return (
    <tr className={selected ? "row-selected" : ""}>
      <td className="col-select" data-label="Select">
        <input
          type="checkbox"
          checked={selected}
          onChange={() => onSelect(r.id)}
          aria-label={`Select ${r.title}`}
        />
      </td>
      <td className="routine-title-cell" data-label="Title">
        <div className="cell-schedule" title={r.title}>
          <RoutineTitle title={r.title} />
        </div>
        <div className="cell-goal" title={r.rel_path}>
          {r.folder ? `/${r.folder}/${r.slug}` : `/${r.slug}`}
        </div>
        {goalFirstLine !== undefined && (
          <div className="cell-goal" title={r.goal ?? ""}>
            {goalFirstLine}
          </div>
        )}
      </td>
      <td className="routine-schedule-cell" data-label="Schedule">
        <div className="cell-schedule">{schedules.join("\n")}</div>
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
        {previewOpen && <FiresPanel schedules={schedules} now={now} />}
      </td>
      <td data-label="Next run">
        <NextRunCell routine={r} now={now} />
      </td>
      <td data-label="Last fire">
        {lastFire === undefined ? (
          <span className="muted">—</span>
        ) : (
          <div className="cell-triggered">
            {manualIsLatest ? "↻" : "⏱"} {reltime(lastFire)}
          </div>
        )}
      </td>
      <td data-label="Run history">
        <RunHistorySparkline runs={runs} />
      </td>
      <td data-label="Agent">
        <span
          className="cell-handler"
          title={r.agent_registered ? "agent registered" : "agent config missing"}
        >
          <span className={r.agent_registered ? "handler-dot ok" : "handler-dot warn"} />
          {r.agent}
        </span>
        {r.model && <div className="cell-goal">{r.model}</div>}
      </td>
      <td data-label="Repos">
        {repos.length === 0 ? (
          "—"
        ) : (
          <span title={repos.map((x) => x.repository).join("\n")}>{repos.length}</span>
        )}
      </td>
      <td data-label="Machines" className={machines.length === 0 ? "cell-meta cell-no-machines" : "cell-meta"}>
        {machines.length === 0 ? "—" : <span title={machines.join("\n")}>{machines.length}</span>}
      </td>
      <td data-label="Tags">{tags.length === 0 ? "—" : <span title={tags.join(", ")}>{tags.join(", ")}</span>}</td>
      <td data-label="TTL">
        <span className="cell-meta" title="workbench retention for finished runs">
          {formatTtl(r.ttl_secs)}
        </span>
      </td>
      <td data-label="Health">
        <span className={healthBadgeClass(health)} title={healthTooltip(r, health)}>
          {healthBadge(health)}
        </span>
        {risk !== "none" && (
          <div
            className={`failure-chip ${risk}`}
            title={`${r.consecutive_failures} consecutive failure(s) — auto-disables at ${r.failure_threshold}`}
          >
            {r.consecutive_failures}/{r.failure_threshold}
          </div>
        )}
      </td>
      <td data-label="Enabled">
        <label className="toggle">
          <input type="checkbox" checked={r.enabled} onChange={(e) => onToggle(r.id, e.target.checked)} />
          <div className="toggle-track" />
        </label>
      </td>
      <td data-label="Updated">
        <div className="cell-time" title={abstime(r.updated_at)}>
          {updated}
        </div>
      </td>
      <td className="routine-actions-cell" data-label="Actions">
        <div className="row-actions">
          <button
            type="button"
            className="act-btn run"
            title={triggerButtonTitle(r)}
            aria-label="Run now"
            disabled={!r.enabled || r.power_saving}
            onClick={() => onTrigger(r.id)}
          >
            RUN
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
          <button type="button" className="act-btn" title="Move folder" onClick={() => onMove(r.id)}>
            MOVE
          </button>
          <button
            type="button"
            className="act-btn clone"
            title="Duplicate routine"
            aria-label="Duplicate routine"
            onClick={() => onClone(r.id)}
          >
            CLONE
          </button>
          <button
            type="button"
            className="act-btn del"
            title="Delete routine"
            aria-label="Delete routine"
            onClick={() => onDelete(r.id, r.title)}
          >
            DELETE
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

function FiresPanel({ schedules, now }: { schedules: string[]; now: Date }) {
  const fires = nextFiresAny(schedules, now, 10);
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

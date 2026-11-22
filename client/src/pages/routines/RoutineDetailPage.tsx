import { Link, useNavigate, useParams } from "react-router-dom";
import { useRoutine, useRoutineRuns, useTriggerRoutine, type RoutineResponse, type RunSummary } from "../../api/hooks";
import { fmtFreshness } from "../../components/RefreshControl";
import { abstime, reltime } from "../../lib/cronUtils";
import { usePins } from "../../lib/pins";
import { fmtRunDuration, runStatusClass, runStatusLabel } from "../../lib/runDisplay";
import { fmtUntil, fmtWhen, nextFiresAny, scheduleList } from "../../lib/schedule";
import { useNow } from "../../lib/useNow";
import { useToasts } from "../../shell/toasts";
import { computeReliability, rateClass, rateLabel, streakClass, streakLabel, successRate } from "../reliability/reliabilityStats";
import { healthBadge, healthBadgeClass, healthTooltip, isRoutineSnoozed, routineHealth, triggerButtonTitle } from "./filter";
import { RunDurationChart } from "./RunDurationChart";
import { formatTtl } from "./ttl";

/** Upcoming fires listed in the UPCOMING panel. */
const UPCOMING_COUNT = 5;
/** Newest runs listed in the RUNS table; the full list stays one click away in history. */
const RECENT_COUNT = 10;

function runPath(id: string, workbench: string): string {
  return `/runs/${encodeURIComponent(id)}/${encodeURIComponent(workbench)}`;
}

function fmtSecs(secs: number | null | undefined): string {
  return secs == null ? "—" : fmtRunDuration(0, secs);
}

/** Reliability metrics for one routine's runs — reuses the fleet math so both pages agree. */
function routineStats(r: RoutineResponse, runs: RunSummary[]) {
  return computeReliability(runs.map((run) => ({ ...run, routine_id: r.id, routine_title: r.title })))[0];
}

/** A `dt`/`dd` pair that renders "—" for an empty value. */
function Field({ label, value }: { label: string; value: string | null | undefined }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>{value == null || value === "" ? "—" : value}</dd>
    </>
  );
}

/**
 * One routine's own addressable page: `/routines/:id`. The per-job landing page job schedulers and
 * cron monitors give every job (Cronitor's monitor page, GitHub Actions' workflow page): health,
 * KPIs, a duration timeline that drills into each run, configuration, upcoming fires and recent
 * runs on one screen, with Run-now and pin as quick actions.
 */
export function RoutineDetailPage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const nowMs = useNow();
  const now = new Date(nowMs);
  const routineQuery = useRoutine(id);
  const runsQuery = useRoutineRuns(id);
  const trigger = useTriggerRoutine();
  const { addToast } = useToasts();
  const { isPinned, toggle: togglePin } = usePins();

  const r = routineQuery.data;

  const header = (
    <div className="page-hd">
      <Link to="/routines" className="btn btn-ghost btn-sm">
        ← ROUTINES
      </Link>
      <div className="page-title">ROUTINE / {r?.title ?? id}</div>
      {routineQuery.dataUpdatedAt > 0 && (
        <span className="page-freshness">{fmtFreshness(Math.max(0, (nowMs - routineQuery.dataUpdatedAt) / 1000))}</span>
      )}
      <button
        type="button"
        className="btn-refresh"
        title="Refresh"
        aria-label="Refresh"
        onClick={() => {
          void routineQuery.refetch();
          void runsQuery.refetch();
        }}
      >
        ↻
      </button>
    </div>
  );

  if (routineQuery.isLoading) {
    return (
      <main className="logs-page">
        {header}
        <div className="empty">
          <div className="spinner" />
        </div>
      </main>
    );
  }
  if (r === undefined) {
    return (
      <main className="logs-page">
        {header}
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⧗</div>
            <div className="empty-msg">ROUTINE NOT FOUND</div>
            <div className="empty-sub">it may have been deleted, or the link is stale</div>
          </div>
        </div>
      </main>
    );
  }

  const runs = runsQuery.data ?? [];
  const stats = routineStats(r, runs);
  const rate = stats ? successRate(stats) : null;
  const lastRun = runs[0];
  const health = routineHealth(r, now);
  const schedules = scheduleList(r);
  const canFire = r.enabled && !isRoutineSnoozed(r, now);
  const upcoming = canFire ? nextFiresAny(schedules, now, UPCOMING_COUNT) : [];
  const next = upcoming[0];
  const descriptions = r.schedule_descriptions ?? [r.schedule_description ?? ""];
  const flags = r.flag_count;
  const triggerDisabled = !r.enabled || r.power_saving === true || r.is_running || trigger.isPending;

  return (
    <main className="logs-page routine-detail">
      {header}

      <div className="rd-actions">
        <span className={healthBadgeClass(health)} title={healthTooltip(r, health)}>
          {healthBadge(health)}
        </span>
        {r.is_running && <span className="run-status running">RUNNING</span>}
        <button
          type="button"
          className="btn btn-sm"
          disabled={triggerDisabled}
          title={r.is_running ? "A run is already in progress" : triggerButtonTitle(r)}
          onClick={() =>
            trigger.mutate(r.id, {
              onSuccess: () => addToast(`Triggered "${r.title}"`, "ok"),
              onError: (e) => addToast(`Trigger failed: ${e.message}`, "err"),
            })
          }
        >
          ▶ RUN NOW
        </button>
        <button type="button" className="btn btn-ghost btn-sm" aria-pressed={isPinned(r.id)} onClick={() => togglePin(r.id)}>
          {isPinned(r.id) ? "★ PINNED" : "☆ PIN"}
        </button>
        <Link to={`/routines?history=${encodeURIComponent(r.id)}`} className="btn btn-ghost btn-sm">
          FULL HISTORY
        </Link>
      </div>

      {r.goal?.trim() && <p className="rd-goal">{r.goal}</p>}

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">LAST RUN</div>
          <div className="stat-val stat-val-sm">
            {lastRun === undefined ? (
              "—"
            ) : (
              <span className={runStatusClass(lastRun.status)} title={abstime(lastRun.started_at)}>
                {runStatusLabel(lastRun.status)}
              </span>
            )}
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-label">SUCCESS RATE</div>
          <div className={`stat-val ${rateClass(rate)}`}>{rateLabel(rate)}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">STREAK</div>
          <div className={`stat-val stat-val-sm ${stats ? streakClass(stats.streak) : ""}`}>
            {stats ? streakLabel(stats.streak) : "—"}
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-label">P50</div>
          <div className="stat-val stat-val-sm">{fmtSecs(stats?.p50Secs)}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">P95</div>
          <div className="stat-val stat-val-sm">
            {fmtSecs(stats?.p95Secs)}
            {stats?.regressing === true && (
              <span className="c-amber" title="Recent runs are meaningfully slower than the older half of the sample">
                {" ▲ slower"}
              </span>
            )}
          </div>
        </div>
        <div className="stat-card stat-card-wide">
          <div className="stat-label">NEXT RUN</div>
          <div className="stat-val stat-val-sm">{next === undefined ? "—" : `${fmtWhen(now, next)} · ${fmtUntil(now, next)}`}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">OPEN FLAGS</div>
          <div className={`stat-val ${flags > 0 ? "c-red" : "c-accent"}`}>
            {flags > 0 ? <Link to="/flags">{flags}</Link> : 0}
          </div>
        </div>
      </div>

      <div className="rd-grid">
        <section className="rd-panel" aria-label="Configuration">
          <h2 className="filter-label rd-h">CONFIGURATION</h2>
          <dl className="rd-fields">
            <dt>Schedule</dt>
            <dd>
              {schedules.map((s, i) => (
                <div key={s}>
                  <code>{s}</code> <span className="muted">{descriptions[i] ?? ""}</span>
                </div>
              ))}
            </dd>
            <Field label="Timezone" value={r.timezone} />
            <Field label="Agent" value={r.model ? `${r.agent} · ${r.model}` : r.agent} />
            <Field label="Machines" value={(r.machines ?? []).join(", ")} />
            <Field label="Repositories" value={(r.repositories ?? []).map((x) => x.repository).join(", ")} />
            <Field label="Tags" value={(r.tags ?? []).join(", ")} />
            <Field label="Retention" value={formatTtl(r.ttl_secs)} />
            <Field label="Max runtime" value={r.max_runtime_secs == null ? null : fmtSecs(r.max_runtime_secs)} />
            <Field label="Env keys" value={r.env_keys.join(", ")} />
            <Field label="Path" value={r.rel_path} />
          </dl>
        </section>

        <section className="rd-panel" aria-label="Upcoming fires">
          <h2 className="filter-label rd-h">UPCOMING</h2>
          {upcoming.length === 0 ? (
            <div className="muted">{canFire ? "no upcoming fires" : "not scheduled while paused or snoozed"}</div>
          ) : (
            <ol className="rd-upcoming">
              {upcoming.map((d) => (
                <li key={d.getTime()}>
                  {fmtWhen(now, d)} <span className="muted">· {fmtUntil(now, d)}</span>
                </li>
              ))}
            </ol>
          )}
        </section>
      </div>

      <h2 className="filter-label rd-h">RUNS</h2>
      {runsQuery.isLoading ? (
        <div className="empty">
          <div className="spinner" />
        </div>
      ) : runsQuery.isError ? (
        <div className="logs-error">Error: {runsQuery.error.message}</div>
      ) : runs.length === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⧗</div>
            <div className="empty-msg">NO RUNS YET</div>
          </div>
        </div>
      ) : (
        <>
          <RunDurationChart runs={runs} selected={undefined} onSelect={(wb) => void navigate(runPath(r.id, wb))} />
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>STARTED</th>
                  <th>STATUS</th>
                  <th>DURATION</th>
                  <th>EXIT CODE</th>
                </tr>
              </thead>
              <tbody>
                {runs.slice(0, RECENT_COUNT).map((run) => (
                  <tr key={run.workbench}>
                    <td title={abstime(run.started_at)}>
                      <Link to={runPath(r.id, run.workbench)}>{reltime(run.started_at)}</Link>
                    </td>
                    <td>
                      <span className={runStatusClass(run.status)}>{runStatusLabel(run.status)}</span>
                    </td>
                    <td>{run.finished_at != null ? fmtRunDuration(run.started_at, run.finished_at) : "—"}</td>
                    <td>{run.exit_code ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </main>
  );
}

import { useState } from "react";
import { useHealth, useSetUserPrompt, useUserPrompt, type HealthResponse } from "../../api/hooks";
import { loadRefreshToken, RefreshControl, saveRefreshToken, type RefreshToken } from "../../components/RefreshControl";
import { loadThemeLight, saveThemeLight } from "../../lib/theme";
import { useToasts } from "../../shell/toasts";

export function SettingsPage() {
  const { addToast } = useToasts();
  const userPrompt = useUserPrompt();
  const setUserPrompt = useSetUserPrompt();
  const health = useHealth(30_000);

  const [content, setContent] = useState("");
  const [loadedContent, setLoadedContent] = useState("");
  const [refreshToken, setRefreshToken] = useState<RefreshToken>(loadRefreshToken);
  const [lightTheme, setLightTheme] = useState(loadThemeLight);

  // Seed the editable draft once the initial fetch resolves. Adjusting state during
  // render (rather than in an effect) on a tracked "previous prop" per
  // https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
  // avoids the extra render pass `react-hooks/set-state-in-effect` warns about.
  const [seededFrom, setSeededFrom] = useState<string | undefined>(undefined);
  if (userPrompt.data !== undefined && userPrompt.data !== seededFrom) {
    setSeededFrom(userPrompt.data);
    setContent(userPrompt.data);
    setLoadedContent(userPrompt.data);
  }

  const dirty = content !== loadedContent;

  const save = () => {
    setUserPrompt.mutate(content, {
      onSuccess: () => {
        setLoadedContent(content);
        addToast("Prompt saved", "ok");
      },
      onError: (err) => addToast(`Save failed: ${err.message}`, "err"),
    });
  };

  const onSetRefreshToken = (next: RefreshToken) => {
    saveRefreshToken(next);
    setRefreshToken(next);
    addToast("Refresh cadence saved", "ok");
  };

  const onSetTheme = (nextLight: boolean) => {
    saveThemeLight(nextLight);
    setLightTheme(nextLight);
    addToast("Theme saved", "ok");
  };

  return (
    <div>
      <h1 className="page-title">Settings</h1>
      <div className="card settings-card">
        <div className="settings-card-title">Appearance</div>
        <p className="settings-card-copy">
          Choose the app theme. This replaces the header theme toggle so appearance lives with the rest of Settings.
        </p>
        <div className="settings-segmented" role="group" aria-label="Theme">
          <button
            type="button"
            className={`btn ${lightTheme ? "btn-primary" : "btn-secondary"}`}
            aria-pressed={lightTheme}
            onClick={() => onSetTheme(true)}
          >
            Light
          </button>
          <button
            type="button"
            className={`btn ${!lightTheme ? "btn-primary" : "btn-secondary"}`}
            aria-pressed={!lightTheme}
            onClick={() => onSetTheme(false)}
          >
            Dark
          </button>
        </div>
      </div>
      <div className="card settings-card">
        <div className="settings-card-title">Data refresh</div>
        <p className="settings-card-copy">
          Choose how often the app pulls fresh daemon data. The same cadence is used by overview,
          routines, logs, heatmap, machines, and reliability views.
        </p>
        <RefreshControl token={refreshToken} updatedAtMs={0} onChange={onSetRefreshToken} />
      </div>
      <div className="card settings-card settings-health-card">
        <div className="settings-card-title">System health</div>
        <CrontabSyncStatus health={health.data} loading={health.isLoading} onRefresh={() => void health.refetch()} />
      </div>
      <div className="card" style={{ padding: 16 }}>
        <div style={{ fontWeight: 700, marginBottom: 4 }}>Persistent prompt</div>
        <p style={{ color: "var(--text-dim)", fontSize: 13, marginTop: 0 }}>
          Appended to every routine&apos;s agent instructions file (CLAUDE.md/AGENTS.md), alongside
          the moadim-managed preamble, on every run.
        </p>
        {userPrompt.isLoading ? (
          <div>Loading…</div>
        ) : (
          <>
            <textarea
              className="form-textarea"
              rows={12}
              placeholder="e.g. always run `cargo fmt` before finishing a task"
              value={content}
              onChange={(e) => setContent(e.target.value)}
            />
            <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 10 }}>
              <button
                className="btn btn-primary"
                disabled={!dirty || setUserPrompt.isPending}
                onClick={save}
              >
                {setUserPrompt.isPending ? "Saving…" : "Save"}
              </button>
              {dirty && <span style={{ color: "var(--text-faint)", fontSize: 12 }}>unsaved changes</span>}
            </div>
          </>
        )}
      </div>
    </div>
  );
}


interface CrontabSyncStatusProps {
  health: HealthResponse | undefined;
  loading: boolean;
  onRefresh: () => void;
}

function CrontabSyncStatus({ health, loading, onRefresh }: CrontabSyncStatusProps) {
  const sync = health?.crontab_sync;
  if (loading && !sync) {
    return <p className="settings-card-copy">Checking OS crontab sync…</p>;
  }

  if (!sync) {
    return (
      <div className="settings-health settings-health-unknown">
        <div className="settings-health-main">Health details unavailable</div>
        <p className="settings-card-copy">Refresh status once the daemon is reachable.</p>
        <button type="button" className="btn btn-secondary" onClick={onRefresh}>
          Refresh status
        </button>
      </div>
    );
  }

  if (sync.ok) {
    return (
      <div className="settings-health settings-health-ok">
        <div className="settings-health-main">OS crontab sync is healthy</div>
        <p className="settings-card-copy">Scheduled routine changes are installed in the OS crontab.</p>
        <button type="button" className="btn btn-secondary" onClick={onRefresh}>
          Refresh status
        </button>
      </div>
    );
  }

  return (
    <div className="settings-health settings-health-warn">
      <div className="settings-health-main">⚠ OS crontab sync needs attention</div>
      <p className="settings-card-copy">
        Scheduled routine changes may not fire until macOS allows Moadim to write the crontab again.
      </p>
      {sync.last_error && <div className="settings-health-error">{sync.last_error}</div>}
      {sync.last_error_at && (
        <div className="settings-health-meta">Last failed: {formatUnixTime(sync.last_error_at)}</div>
      )}
      <ol className="settings-recovery-list" aria-label="Crontab sync recovery steps">
        <li>Open macOS System Settings → Privacy & Security → Full Disk Access.</li>
        <li>Allow the Moadim daemon or the launcher that starts it.</li>
        <li>Restart Moadim, then update or save a routine to retry crontab sync.</li>
      </ol>
      <button type="button" className="btn btn-secondary" onClick={onRefresh}>
        Refresh status
      </button>
    </div>
  );
}

function formatUnixTime(unixSecs: number): string {
  return new Date(unixSecs * 1000).toLocaleString();
}

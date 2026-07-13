import type { HealthResponse } from "../api/hooks";

function fmtUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3_600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3_600)}h ${Math.floor((secs % 3_600) / 60)}m`;
}

export interface HeaderProps {
  health: HealthResponse | undefined;
  healthOk: boolean;
  light: boolean;
  machineName: string | undefined;
  onRefresh: () => void;
  onStop: () => void;
  onPalette: () => void;
  onTheme: () => void;
  onRenameMachine: () => void;
}

export function Header(props: HeaderProps) {
  const { health } = props;
  const status = health?.status.toUpperCase() ?? "UNKNOWN";
  const version = health?.version ? `/ v${health.version}` : "";
  const versionTitle =
    health?.git_sha && health.git_sha !== "unknown" ? `build: ${health.git_sha}` : undefined;
  const uptime = health?.uptime_secs !== undefined && health.uptime_secs !== null ? `/ UP ${fmtUptime(health.uptime_secs)}` : "";
  const missingTmux = health?.dependencies ? !health.dependencies.tmux : false;
  const missingPython3 = health?.dependencies ? !health.dependencies.python3 : false;

  return (
    <header className="app-header">
      <span className="brand">
        MOADIM<span className="brand-sub">/ control</span>
        {version && (
          <span className="brand-sub" title={versionTitle}>
            {version}
          </span>
        )}
      </span>
      <div className="header-spacer" />
      <div className="header-right">
        {missingTmux && (
          <span
            className="dep-warn"
            title="tmux is not on the daemon's PATH — all routine runs will silently fail"
          >
            ⚠ NO TMUX
          </span>
        )}
        {missingPython3 && (
          <span
            className="dep-warn"
            title="python3 is not on the daemon's PATH — the claude agent setup step will fail silently"
          >
            ⚠ NO PYTHON3
          </span>
        )}
        <div className="health">
          <div className={`health-dot ${props.healthOk ? "ok" : "error"}`} />
          <span>{status}</span>
          <span>{uptime}</span>
        </div>
        {props.machineName && (
          <button
            className="machine-badge"
            title="Click to rename this machine"
            onClick={props.onRenameMachine}
          >
            {props.machineName}
          </button>
        )}
        <button
          className="icon-btn"
          title={props.light ? "Switch to dark mode" : "Switch to light mode"}
          onClick={props.onTheme}
        >
          {props.light ? "☀" : "🌙"}
        </button>
        <button className="icon-btn" title="Command palette (⌘K)" onClick={props.onPalette}>
          ⌘K
        </button>
        <button className="icon-btn" title="Refresh" onClick={props.onRefresh}>
          ↻
        </button>
        <button
          className="icon-btn danger"
          title="Stop the server"
          disabled={!props.healthOk}
          onClick={props.onStop}
        >
          ⏻ Stop
        </button>
      </div>
    </header>
  );
}

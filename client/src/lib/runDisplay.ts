import type { RunSummary } from "../api/hooks";

type RunStatus = RunSummary["status"];

/** CSS class for a run's status badge. */
export function runStatusClass(status: RunStatus): string {
  return `run-status ${status}`;
}

/** Display label for a run's status badge. */
export function runStatusLabel(status: RunStatus): string {
  switch (status) {
    case "running":
      return "RUNNING";
    case "success":
      return "SUCCESS";
    case "failed":
      return "FAILED";
    case "killed":
      return "KILLED";
    default:
      return "UNKNOWN";
  }
}

/** Human-readable reason a run was killed, for a status tooltip; `null` when none was recorded. */
export function killReasonLabel(reason: RunSummary["kill_reason"]): string | null {
  switch (reason) {
    case "max_runtime":
      return "exceeded max runtime";
    default:
      return null;
  }
}

/** Wall-clock duration between a run's start and finish as `"<n>s"`/`"<n>m"`/`"<n>h <n>m"`. */
export function fmtRunDuration(startedAt: number, finishedAt: number): string {
  const secs = Math.max(0, finishedAt - startedAt);
  if (secs < 60) return `${secs}s`;
  if (secs < 3_600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3_600)}h ${Math.floor((secs % 3_600) / 60)}m`;
}

/**
 * Humanized countdown to when a finished run's workbench is due to be reaped.
 * Reads `"expired"` once the deadline has passed rather than a negative countdown.
 */
export function fmtRetention(now: number, expiresAt: number): string {
  if (now >= expiresAt) return "expired";
  const secs = expiresAt - now;
  if (secs < 60) return "expires in <1m";
  if (secs < 3_600) return `expires in ${Math.floor(secs / 60)}m`;
  return `expires in ${Math.floor(secs / 3_600)}h ${Math.floor((secs % 3_600) / 60)}m`;
}

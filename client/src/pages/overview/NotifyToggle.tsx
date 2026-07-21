export interface NotifyToggleProps {
  enabled: boolean;
  onToggle: (next: boolean) => void;
}

/** Opt-in toggle for desktop notifications on a freshly-failed run, next to the recent-runs refresh control. */
export function NotifyToggle({ enabled, onToggle }: NotifyToggleProps) {
  return (
    <button
      type="button"
      className={enabled ? "btn btn-sm btn-ghost notify-toggle active" : "btn btn-sm btn-ghost notify-toggle"}
      title={enabled ? "Desktop notifications on failure: on (click to turn off)" : "Get a desktop notification when a run fails"}
      aria-pressed={enabled}
      onClick={() => onToggle(!enabled)}
    >
      {enabled ? "🔔 Notify on failure" : "🔕 Notify on failure"}
    </button>
  );
}

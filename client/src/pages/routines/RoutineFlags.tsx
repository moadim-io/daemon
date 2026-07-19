import { useFlags, useResolveFlag } from "../../api/hooks";
import { fmtFreshness } from "../../components/RefreshControl";
import { reltime } from "../../lib/cronUtils";
import { useNow } from "../../lib/useNow";

export interface RoutineFlagsProps {
  id: string;
  title: string;
  onBack: () => void;
}

/** Open-flags panel for one routine, with a RESOLVE action per flag. */
export function RoutineFlags({ id, title, onBack }: RoutineFlagsProps) {
  const flagsQuery = useFlags(id);
  const resolveFlag = useResolveFlag();
  const now = useNow();

  const flags = flagsQuery.data ?? [];

  return (
    <main className="logs-page">
      <div className="page-hd">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onBack}>
          ← BACK
        </button>
        <div className="page-title">FLAGS / {title}</div>
        {flagsQuery.dataUpdatedAt > 0 && (
          <span className="page-freshness">
            {fmtFreshness(Math.max(0, (now - flagsQuery.dataUpdatedAt) / 1000))}
          </span>
        )}
        <button
          type="button"
          className="btn-refresh"
          title="Refresh"
          aria-label="Refresh"
          onClick={() => void flagsQuery.refetch()}
        >
          ↻
        </button>
      </div>

      {flagsQuery.isLoading ? (
        <div className="empty">
          <div className="spinner" />
        </div>
      ) : flagsQuery.isError ? (
        <div className="logs-error">{flagsQuery.error.message}</div>
      ) : flags.length === 0 ? (
        <div className="empty">
          <div className="empty-icon">⚑</div>
          <div className="empty-msg">NO OPEN FLAGS</div>
        </div>
      ) : (
        <div className="flags-list">
          <div className="flags-count">
            {flags.length} open flag{flags.length === 1 ? "" : "s"}
          </div>
          {flags.map((flag) => (
            <div className="flag-item" key={flag.filename}>
              <div className="flag-item-hd">
                <span className="flag-type">{flag.type}</span>
                <span className="flag-scope">{flag.scope === "general" ? "general" : "local"}</span>
                <span className="flag-age" title={flag.filename}>
                  {reltime(flag.created_at)}
                </span>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  disabled={resolveFlag.isPending}
                  onClick={() => resolveFlag.mutate({ id, filename: flag.filename })}
                >
                  RESOLVE
                </button>
              </div>
              <div className="flag-desc">{flag.description}</div>
            </div>
          ))}
        </div>
      )}
    </main>
  );
}

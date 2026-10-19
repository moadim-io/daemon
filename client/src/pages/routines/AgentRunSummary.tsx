export interface AgentRunSummaryProps {
  /** Raw `summary.md` text; `undefined` while loading. Empty string means the agent wrote none. */
  content: string | undefined;
  loading: boolean;
  err: string | undefined;
}

/**
 * The agent-authored `summary.md` for one run — a human-written recap of what happened,
 * shown above the raw log the same way CI tools (GitHub Actions job summaries, CircleCI)
 * surface a run's headline result before its full output.
 */
export function AgentRunSummary({ content, loading, err }: AgentRunSummaryProps) {
  let body: React.ReactNode;
  if (loading) {
    body = (
      <div className="empty">
        <div className="spinner" />
      </div>
    );
  } else if (err !== undefined) {
    body = <div className="logs-error">Error: {err}</div>;
  } else if (content === undefined) {
    return null;
  } else if (content === "") {
    body = <div className="logs-empty">— the agent didn&apos;t write a summary for this run —</div>;
  } else {
    body = <div className="run-summary-text">{content}</div>;
  }

  return (
    <div className="run-summary">
      <div className="section-hd">
        <span className="section-label">SUMMARY</span>
      </div>
      <div className="logs-wrap run-summary-wrap">{body}</div>
    </div>
  );
}

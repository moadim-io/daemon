import { useEffect, useMemo, useRef, useState } from "react";
import { filterLines, highlightSegments, linesOf, matchCount } from "./logSearch";
import { severityCounts, severityOf, stepProblem } from "./logSeverity";

export interface LogViewerProps {
  /** Raw log text; `undefined` while loading. */
  content: string | undefined;
  loading: boolean;
  err: string | undefined;
}

function HighlightedLine({ text, query }: { text: string; query: string }) {
  return (
    <>
      {highlightSegments(text, query).map(([isMatch, slice], i) =>
        isMatch ? (
          <mark key={i} className="log-hl">
            {slice}
          </mark>
        ) : (
          <span key={i}>{slice}</span>
        ),
      )}
    </>
  );
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/**
 * Scrollable log body with a live search box (filter + count + highlight), auto-tail, and severity
 * triage: error/warning lines are color-coded and counted, can be filtered to ("Problems only"),
 * and stepped through with Prev / Next.
 */
export function LogViewer({ content, loading, err }: LogViewerProps) {
  const [query, setQuery] = useState("");
  const [problemsOnly, setProblemsOnly] = useState(false);
  const [cursor, setCursor] = useState<number | undefined>(undefined);
  const wrapRef = useRef<HTMLDivElement>(null);

  // Auto-tail: scroll to the bottom whenever the content changes, so newest lines stay visible.
  useEffect(() => {
    const el = wrapRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [content?.length]);

  // Bring the selected problem line into view.
  useEffect(() => {
    if (cursor === undefined) return;
    wrapRef.current?.querySelector(`[data-ln="${cursor}"]`)?.scrollIntoView({ block: "center" });
  }, [cursor]);

  const counts = useMemo(() => severityCounts(linesOf(content ?? "")), [content]);

  let body: React.ReactNode = null;
  if (loading) {
    body = (
      <div className="empty">
        <div className="spinner" />
      </div>
    );
  } else if (err !== undefined) {
    body = <div className="logs-error">Error: {err}</div>;
  } else if (content !== undefined) {
    if (content === "") {
      body = <div className="logs-empty">— no logs yet —</div>;
    } else {
      const lines = filterLines(content, query)
        .map(([ln, text]) => [ln, text, severityOf(text)] as const)
        .filter(([, , sev]) => !problemsOnly || sev !== undefined);
      const problems = lines.filter(([, , sev]) => sev !== undefined).map(([ln]) => ln);
      const [hits, total] = matchCount(content, query);
      const countLabel = query.trim() === "" ? `${total} lines` : `${hits} / ${total} matches`;
      const step = (dir: 1 | -1) => setCursor(stepProblem(problems, cursor, dir));
      body = (
        <>
          <div className="log-search">
            <input
              type="search"
              className="log-search-input"
              placeholder="Search logs…"
              aria-label="Search log lines"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            {query !== "" && (
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                title="Clear search"
                aria-label="Clear search"
                onClick={() => setQuery("")}
              >
                ✕
              </button>
            )}
            <span className="log-match-count">{countLabel}</span>
          </div>
          <div className="log-triage" role="toolbar" aria-label="Log problems">
            <span className={`log-sev-chip log-sev-chip-error${counts.error ? "" : " is-zero"}`}>
              {plural(counts.error, "error")}
            </span>
            <span className={`log-sev-chip log-sev-chip-warn${counts.warn ? "" : " is-zero"}`}>
              {plural(counts.warn, "warning")}
            </span>
            <label className="log-triage-toggle">
              <input type="checkbox" checked={problemsOnly} onChange={(e) => setProblemsOnly(e.target.checked)} />
              Problems only
            </label>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              disabled={problems.length === 0}
              aria-label="Previous problem"
              title="Previous error or warning"
              onClick={() => step(-1)}
            >
              ↑ Prev
            </button>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              disabled={problems.length === 0}
              aria-label="Next problem"
              title="Next error or warning"
              onClick={() => step(1)}
            >
              ↓ Next
            </button>
          </div>
          <div className="log-lines">
            {lines.length === 0 && <div className="logs-empty">— no matching lines —</div>}
            {lines.map(([ln, text, sev]) => (
              <div
                className={`log-line${sev ? ` log-sev-${sev}` : ""}${ln === cursor ? " log-line-current" : ""}`}
                key={ln}
                data-ln={ln}
                aria-current={ln === cursor ? "true" : undefined}
              >
                <span className="log-ln">{ln}</span>
                <span className="log-lc">
                  <HighlightedLine text={text} query={query} />
                </span>
              </div>
            ))}
          </div>
        </>
      );
    }
  }

  return (
    <div className="logs-wrap" ref={wrapRef}>
      {body}
    </div>
  );
}

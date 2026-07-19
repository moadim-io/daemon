import { useEffect, useRef, useState } from "react";
import { filterLines, highlightSegments, matchCount } from "./logSearch";

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

/** Scrollable log body with a live search box (filter + count + highlight) and auto-tail. */
export function LogViewer({ content, loading, err }: LogViewerProps) {
  const [query, setQuery] = useState("");
  const wrapRef = useRef<HTMLDivElement>(null);

  // Auto-tail: scroll to the bottom whenever the content changes, so newest lines stay visible.
  useEffect(() => {
    const el = wrapRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [content?.length]);

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
      const lines = filterLines(content, query);
      const [hits, total] = matchCount(content, query);
      const countLabel = query.trim() === "" ? `${total} lines` : `${hits} / ${total} matches`;
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
          <div className="log-lines">
            {lines.map(([ln, text]) => (
              <div className="log-line" key={ln}>
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

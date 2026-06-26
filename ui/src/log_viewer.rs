//! Reusable enhanced log viewer: line numbers, keyword search/highlight, and auto-tail.
//!
//! Best-practice for ops/CI dashboards (GitLab, Harness, Datadog): a log panel must let
//! operators zero in on errors without manual scrolling.  Three patterns drive this:
//!
//! 1. **Line numbers** — anchor copy-paste references and support "go to line N" mental model.
//! 2. **Keyword search + highlight** — filter to matching lines and highlight the term, with
//!    a live `N / total` count so the operator knows the scope of a hit.
//! 3. **Auto-tail** — scroll to bottom on content load so the most-recent entry is visible
//!    immediately, matching the convention used by GitLab CI, Harness, and Buildkite.

use web_sys::{Element, HtmlInputElement};
use yew::prelude::*;

// ─── Pure helpers (host-testable, no DOM/wasm dependency) ─────────────────────

/// Return `(1-based line number, line text)` pairs from `content`.
///
/// When `query` is non-empty after trimming, only lines whose lowercase text
/// contains the lowercase query are returned — preserving their original numbers
/// so the caller can anchor to the source.
#[must_use]
pub fn filter_lines<'a>(content: &'a str, query: &str) -> Vec<(usize, &'a str)> {
    let needle = query.trim().to_lowercase();
    content
        .lines()
        .enumerate()
        .filter(|(_, line)| needle.is_empty() || line.to_lowercase().contains(&needle))
        .map(|(i, line)| (i + 1, line))
        .collect()
}

/// Return `(matching_lines, total_lines)` for `content` against `query`.
///
/// When `query` is blank (empty or whitespace-only), `matching == total`.
#[must_use]
pub fn match_count(content: &str, query: &str) -> (usize, usize) {
    let total = content.lines().count();
    if query.trim().is_empty() {
        return (total, total);
    }
    let needle = query.trim().to_lowercase();
    let hits = content
        .lines()
        .filter(|l| l.to_lowercase().contains(&needle))
        .count();
    (hits, total)
}

// ─── Yew component ────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct LogViewerProps {
    /// Raw log text, or `None` while the fetch is in flight.
    pub content: Option<String>,
    pub loading: bool,
    pub err: Option<String>,
}

/// Log viewer with line numbers, keyword search/highlight, and auto-tail.
///
/// Renders a sticky search bar above the numbered log body.  Typing a query
/// narrows the display to matching lines (with the term highlighted) and shows a
/// live `N / total` count.  Content changes trigger an automatic scroll to the
/// bottom so the latest log entry is always visible without manual scrolling.
#[function_component(LogViewer)]
pub fn log_viewer(props: &LogViewerProps) -> Html {
    let query = use_state(String::new);
    let wrap_ref = use_node_ref();

    // Auto-tail: scroll the wrapper to its bottom whenever content arrives or changes.
    {
        let wrap_ref = wrap_ref.clone();
        let content_len = props.content.as_ref().map(String::len).unwrap_or(0);
        use_effect_with(content_len, move |_| {
            if let Some(el) = wrap_ref.cast::<Element>() {
                el.set_scroll_top(el.scroll_height());
            }
        });
    }

    let on_query = {
        let query = query.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            query.set(input.value());
        })
    };
    let on_clear = {
        let query = query.clone();
        Callback::from(move |_: MouseEvent| query.set(String::new()))
    };

    let body = if props.loading {
        html! { <div class="empty"><div class="spinner"></div></div> }
    } else if let Some(e) = &props.err {
        html! { <div class="logs-error">{format!("Error: {e}")}</div> }
    } else if let Some(text) = &props.content {
        if text.is_empty() {
            html! { <div class="logs-empty">{"— no logs yet —"}</div> }
        } else {
            let q = (*query).clone();
            let lines = filter_lines(text, &q);
            let (hits, total) = match_count(text, &q);
            let count_label = if q.trim().is_empty() {
                format!("{total} lines")
            } else {
                format!("{hits} / {total} matches")
            };
            let clear_btn = if q.is_empty() {
                html! {}
            } else {
                html! {
                    <button class="btn btn-ghost btn-sm" onclick={on_clear}
                        title="Clear search" aria-label="Clear search">{"✕"}</button>
                }
            };
            html! {
                <>
                    <div class="log-search">
                        <input
                            type="search"
                            class="log-search-input"
                            placeholder="Search logs…"
                            value={q.clone()}
                            oninput={on_query}
                            aria-label="Search log lines"
                        />
                        {clear_btn}
                        <span class="log-match-count">{count_label}</span>
                    </div>
                    <div class="log-lines">
                        { for lines.iter().map(|(ln, lc)| render_line(*ln, lc, &q)) }
                    </div>
                </>
            }
        }
    } else {
        html! {}
    };

    html! {
        <div class="logs-wrap" ref={wrap_ref}>
            {body}
        </div>
    }
}

/// Render one numbered log line, with `query` occurrences highlighted.
fn render_line(ln: usize, text: &str, query: &str) -> Html {
    html! {
        <div class="log-line">
            <span class="log-ln">{ln}</span>
            <span class="log-lc">{ highlight(text, query) }</span>
        </div>
    }
}

/// Split `text` into `Html` with each case-insensitive occurrence of `query`
/// wrapped in `<mark class="log-hl">`.  Returns plain text when `query` is blank.
///
/// Note: byte-offset arithmetic is correct for ASCII log content (the primary use
/// case).  Non-ASCII queries that change byte length when lowercased are guarded
/// by the `end > text.len()` safety check.
fn highlight(text: &str, query: &str) -> Html {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return html! { {text} };
    }
    let lower = text.to_lowercase();
    let mut parts: Vec<Html> = Vec::new();
    let mut start = 0usize;
    while start <= text.len() {
        match lower[start..].find(&needle) {
            None => break,
            Some(pos) => {
                let abs = start + pos;
                let end = abs + needle.len();
                if end > text.len() {
                    break;
                }
                if abs > start {
                    let before = &text[start..abs];
                    parts.push(html! { {before} });
                }
                let matched = &text[abs..end];
                parts.push(html! { <mark class="log-hl">{matched}</mark> });
                start = end;
            }
        }
    }
    if start < text.len() {
        parts.push(html! { {&text[start..]} });
    }
    html! { <>{for parts.into_iter()}</> }
}

#[cfg(test)]
#[path = "log_viewer_tests.rs"]
mod tests;

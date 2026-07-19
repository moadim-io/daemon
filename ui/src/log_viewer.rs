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

#[derive(Properties, PartialEq, Eq)]
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
        let content_len = props.content.as_ref().map_or(0, String::len);
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

/// Split `text` into `(is_match, segment)` pairs, `true` for each case-insensitive occurrence of
/// `query`, in original order; `false` segments fill the gaps. Blank `query` (after trimming)
/// yields the whole text as a single non-matching segment.
///
/// Every slice boundary comes from `text.char_indices()`, so it is always a valid char boundary in
/// `text`. This matters because a naive approach — find the match in `text.to_lowercase()` and
/// reapply *its* byte offsets to `text` — panics on input like `text = "ẞzz"`, `query = "zz"`:
/// `ẞ` (U+1E9E, 3 bytes) lowercases to `ß` (U+00DF, 2 bytes), so the byte offset found in the
/// lowercased string no longer lands on a char boundary in the original. Some chars also *expand*
/// under `to_lowercase()` (Turkish `İ` → `i̇`, two chars), which a byte-length guard alone can't
/// catch either. Instead, each char of `text` is projected to exactly one lowercase char (dropping
/// any expansion beyond the first), keeping a 1:1 index correspondence between `chars` and `lower`
/// so every match position maps back to an exact, valid byte span in `text`.
#[must_use]
fn highlight_segments<'a>(text: &'a str, query: &str) -> Vec<(bool, &'a str)> {
    let needle: Vec<char> = query.trim().to_lowercase().chars().collect();
    if needle.is_empty() {
        return vec![(false, text)];
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|&(_, c)| c.to_lowercase().next().unwrap_or(c))
        .collect();

    let mut segments = Vec::new();
    let mut cursor = 0_usize; // char index into `chars`/`lower`
    let mut last_byte = 0_usize; // byte offset in `text` already emitted
    while cursor + needle.len() <= lower.len() {
        if lower[cursor..cursor + needle.len()] == needle[..] {
            let match_start = chars[cursor].0;
            let match_end = chars
                .get(cursor + needle.len())
                .map_or(text.len(), |&(byte, _)| byte);
            if match_start > last_byte {
                segments.push((false, &text[last_byte..match_start]));
            }
            segments.push((true, &text[match_start..match_end]));
            last_byte = match_end;
            cursor += needle.len();
        } else {
            cursor += 1;
        }
    }
    if last_byte < text.len() {
        segments.push((false, &text[last_byte..]));
    }
    segments
}

/// Render `text` as `Html`, wrapping each case-insensitive occurrence of `query` in
/// `<mark class="log-hl">`. Returns plain text when `query` is blank. See
/// [`highlight_segments`] for the (host-testable) matching logic.
fn highlight(text: &str, query: &str) -> Html {
    html! {
        <>
            { for highlight_segments(text, query).into_iter().map(|(is_match, seg)| {
                if is_match {
                    html! { <mark class="log-hl">{seg}</mark> }
                } else {
                    html! { {seg} }
                }
            }) }
        </>
    }
}

#[cfg(test)]
#[path = "log_viewer_tests.rs"]
mod tests;

//! Shared enhanced log viewer: line numbers, keyword search, and auto-tail.
//!
//! Best practice (GitHub Actions, Grafana Loki, Buildkite): live tailing is the
//! #1 operator ask; line numbers turn a log wall into a navigable document;
//! inline keyword search removes the terminal grep round-trip.
//!
//! `split_lines` and `line_counts` are pure and host-testable (see
//! `logs_tests.rs`); only `LogViewer` and `fetch_log` touch the network layer.

use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

/// Split `text` into `(1-based line number, line content, matches_search)` tuples.
/// When `query` is blank (empty or whitespace-only) every line is marked matching.
pub(crate) fn split_lines<'a>(text: &'a str, query: &str) -> Vec<(usize, &'a str, bool)> {
    let q = query.trim().to_ascii_lowercase();
    text.lines()
        .enumerate()
        .map(|(i, line)| {
            let m = q.is_empty() || line.to_ascii_lowercase().contains(q.as_str());
            (i + 1, line, m)
        })
        .collect()
}

/// Returns `(total_lines, matching_lines)` for a keyword search over `text`.
/// When `query` is blank every line is counted as matching.
pub(crate) fn line_counts(text: &str, query: &str) -> (usize, usize) {
    let q = query.trim().to_ascii_lowercase();
    let total = text.lines().count();
    if q.is_empty() {
        (total, total)
    } else {
        let matching = text
            .lines()
            .filter(|l| l.to_ascii_lowercase().contains(q.as_str()))
            .count();
        (total, matching)
    }
}

// ─── Network ──────────────────────────────────────────────────────────────────

async fn fetch_log(url: &str) -> Result<String, String> {
    let resp = Request::get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    resp.text().await.map_err(|e| e.to_string())
}

// ─── Component ────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct LogViewerProps {
    /// Full log-endpoint URL, e.g. `/api/v1/routines/{id}/logs`.
    pub fetch_url: AttrValue,
    /// Shown in the page header as `LOGS / {title}`.
    pub title: AttrValue,
    pub on_back: Callback<()>,
}

#[function_component(LogViewer)]
pub fn log_viewer(props: &LogViewerProps) -> Html {
    let content: UseStateHandle<Option<String>> = use_state(|| None);
    let loading = use_state(|| true);
    let err: UseStateHandle<Option<String>> = use_state(|| None);
    let tail = use_state(|| false);
    let search = use_state(String::new);
    // Monotonic counter bumped on each successful fetch; cheap scroll-trigger dep.
    let version = use_state(|| 0u32);
    let log_ref = use_node_ref();

    // Shared fetch callback. `set_loading` shows the spinner (initial load /
    // manual refresh); auto-tail silent-polls with `set_loading = false`.
    let load = {
        let content = content.clone();
        let loading = loading.clone();
        let err = err.clone();
        let version = version.clone();
        let url = props.fetch_url.clone();
        move |set_loading: bool| {
            let content = content.clone();
            let loading = loading.clone();
            let err = err.clone();
            let version = version.clone();
            let url = url.clone();
            if set_loading {
                loading.set(true);
            }
            spawn_local(async move {
                match fetch_log(&url).await {
                    Ok(text) => {
                        content.set(Some(text));
                        err.set(None);
                        version.set(*version + 1);
                    }
                    Err(e) => err.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };

    // Reset transient state and fetch fresh content when the URL changes.
    {
        let load = load.clone();
        let content = content.clone();
        let err = err.clone();
        let search = search.clone();
        let tail = tail.clone();
        let version = version.clone();
        use_effect_with(props.fetch_url.clone(), move |_| {
            content.set(None);
            err.set(None);
            search.set(String::new());
            tail.set(false);
            version.set(0);
            load(true);
        });
    }

    let is_tail = *tail;
    let version_val = *version;

    // Scroll the log container to the bottom when new content arrives and tail
    // is active. The `version` dep fires only on actual content changes, not
    // on every re-render.
    {
        let log_ref = log_ref.clone();
        use_effect_with((version_val, is_tail), move |_| {
            if is_tail {
                if let Some(el) = log_ref.cast::<web_sys::Element>() {
                    el.set_scroll_top(el.scroll_height());
                }
            }
        });
    }

    // Auto-tail polling loop: fires every 2 s while tail is on; self-terminates
    // when the operator disables tail (checked at the top of each iteration so
    // the loop exits within one sleep after the toggle).
    {
        let load = load.clone();
        let tail = tail.clone();
        use_effect_with(is_tail, move |is_tailing| {
            if !*is_tailing {
                return;
            }
            let load = load.clone();
            let tail = tail.clone();
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(2_000).await;
                    if !*tail {
                        break;
                    }
                    load(false);
                }
            });
        });
    }

    let on_back = {
        let cb = props.on_back.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_refresh = {
        let load = load.clone();
        Callback::from(move |_: MouseEvent| load(true))
    };
    let on_toggle_tail = {
        let load = load.clone();
        let tail = tail.clone();
        Callback::from(move |_: MouseEvent| {
            let new_tail = !*tail;
            tail.set(new_tail);
            if new_tail {
                // Fetch immediately so the operator sees fresh content right away.
                load(false);
            }
        })
    };
    let on_search = {
        let search = search.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            search.set(input.value());
        })
    };

    let is_loading = *loading;
    let search_q = (*search).clone();

    let body = if is_loading && (*content).is_none() {
        html! { <div class="empty"><div class="spinner"></div></div> }
    } else if let Some(ref e) = *err {
        html! { <div class="logs-error">{format!("Error: {e}")}</div> }
    } else if let Some(ref text) = *content {
        if text.is_empty() {
            html! { <div class="logs-empty">{"— no logs yet —"}</div> }
        } else {
            let lines = split_lines(text, &search_q);
            let (total, matching) = line_counts(text, &search_q);
            let count_label = if search_q.trim().is_empty() {
                format!("{total} lines")
            } else {
                format!("{matching}/{total} lines")
            };
            html! {
                <>
                    <div class="logs-toolbar">
                        <span class="logs-count">{count_label}</span>
                        {
                            if is_tail {
                                html! { <span class="logs-live">{"● LIVE"}</span> }
                            } else {
                                html! {}
                            }
                        }
                        <input
                            class="logs-search"
                            type="search"
                            placeholder="Search logs…"
                            value={search_q.clone()}
                            oninput={on_search}
                            aria-label="Search log lines"
                        />
                    </div>
                    <table class="logs-table" aria-label="Log output">
                        <tbody>
                            { for lines.iter().map(|(n, line, matches)| {
                                let row_cls = if *matches { "logs-row" } else { "logs-row dimmed" };
                                html! {
                                    <tr class={row_cls} key={*n}>
                                        <td class="logs-gutter" aria-hidden="true">{n}</td>
                                        <td class="logs-line">{*line}</td>
                                    </tr>
                                }
                            }) }
                        </tbody>
                    </table>
                </>
            }
        }
    } else {
        html! {}
    };

    let tail_cls = if is_tail {
        "btn btn-sm logs-tail-btn active"
    } else {
        "btn btn-sm logs-tail-btn"
    };

    html! {
        <main class="logs-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_back}>{"← BACK"}</button>
                <div class="page-title">{format!("LOGS / {}", props.title)}</div>
                <div class="logs-hd-acts">
                    <button
                        class={tail_cls}
                        title="Auto-tail: poll every 2s and scroll to bottom"
                        aria-pressed={is_tail.to_string()}
                        onclick={on_toggle_tail}
                    >{"⬇ TAIL"}</button>
                    {
                        if !is_tail {
                            html! {
                                <button
                                    class="btn-refresh"
                                    title="Refresh"
                                    aria-label="Refresh"
                                    onclick={on_refresh}
                                >{"↻"}</button>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
            <div class="logs-wrap" ref={log_ref}>
                {body}
            </div>
        </main>
    }
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod logs_tests;
